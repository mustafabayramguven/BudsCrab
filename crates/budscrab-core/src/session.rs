//! The stateful part: one `Session` per app process. Kotlin owns the socket,
//! feeds received bytes in, and writes out whatever `CoreEvent::Send` says.
//! Every method takes `now_ms` so the logic is deterministic and testable.

use std::collections::VecDeque;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use budscrab_proto::{self as p, ids, Command, FrameDecoder, Incoming};

use crate::types::*;

const PACKET_LOG_CAP: usize = 400;
const DIAG_CAP: usize = 200;
const BATTERY_CAP: usize = 6000;
const BATTERY_MIN_INTERVAL_MS: i64 = 5 * 60 * 1000;

#[derive(uniffi::Object)]
pub struct Session {
    inner: Mutex<Inner>,
}

struct PendingHold {
    side: BudSide,
    count: u32,
    last_ms: i64,
}

struct Inner {
    data_dir: Option<PathBuf>,
    config: Config,
    state: DeviceState,
    decoder: FrameDecoder,
    packets: VecDeque<PacketEntry>,
    diag: VecDeque<DiagEntry>,
    battery: Vec<BatterySample>,
    pending_hold: Option<PendingHold>,
    last_placement_change_ms: i64,
    last_app_playpause_ms: i64,
    last_disconnect_ms: i64,
    low_battery_fired: [bool; 2],
    current_ssid: Option<String>,
    touch_enforced_this_connection: bool,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ")
}

impl Inner {
    fn new(data_dir: Option<PathBuf>) -> Self {
        let mut s = Inner {
            data_dir,
            config: Config::default(),
            state: DeviceState::default(),
            decoder: FrameDecoder::default(),
            packets: VecDeque::new(),
            diag: VecDeque::new(),
            battery: Vec::new(),
            pending_hold: None,
            last_placement_change_ms: i64::MIN / 2,
            last_app_playpause_ms: i64::MIN / 2,
            last_disconnect_ms: i64::MIN / 2,
            low_battery_fired: [false; 2],
            current_ssid: None,
            touch_enforced_this_connection: false,
        };
        s.load();
        s
    }

    // ---- persistence -------------------------------------------------------

    fn path(&self, name: &str) -> Option<PathBuf> {
        self.data_dir.as_ref().map(|d| d.join(name))
    }

    fn load(&mut self) {
        if let Some(path) = self.path("config.json") {
            if let Ok(txt) = fs::read_to_string(path) {
                if let Ok(c) = serde_json::from_str::<Config>(&txt) {
                    self.config = c;
                }
            }
        }
        if let Some(path) = self.path("battery.jsonl") {
            if let Ok(txt) = fs::read_to_string(&path) {
                self.battery = txt.lines().filter_map(|l| serde_json::from_str(l).ok()).collect();
                if self.battery.len() > BATTERY_CAP {
                    let drop = self.battery.len() - BATTERY_CAP;
                    self.battery.drain(..drop);
                    // compact the file
                    let body: String = self
                        .battery
                        .iter()
                        .filter_map(|s| serde_json::to_string(s).ok())
                        .map(|l| l + "\n")
                        .collect();
                    let _ = fs::write(path, body);
                }
            }
        }
    }

    fn save_config(&self) {
        if let Some(path) = self.path("config.json") {
            if let Some(dir) = path.parent() {
                let _ = fs::create_dir_all(dir);
            }
            if let Ok(txt) = serde_json::to_string_pretty(&self.config) {
                let tmp = path.with_extension("json.tmp");
                if fs::write(&tmp, txt).is_ok() {
                    let _ = fs::rename(tmp, path);
                }
            }
        }
    }

    fn append_battery(&mut self, s: BatterySample) {
        if let Some(path) = self.path("battery.jsonl") {
            if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(path) {
                if let Ok(line) = serde_json::to_string(&s) {
                    let _ = writeln!(f, "{line}");
                }
            }
        }
        self.battery.push(s);
        if self.battery.len() > BATTERY_CAP {
            self.battery.remove(0);
        }
    }

    // ---- logging -----------------------------------------------------------

    fn log_packet(&mut self, now: i64, outgoing: bool, raw: &[u8], id: u8) {
        self.packets.push_back(PacketEntry {
            ts_ms: now,
            outgoing,
            id: id as i32,
            name: ids::name(id).to_string(),
            hex: hex(raw),
        });
        while self.packets.len() > PACKET_LOG_CAP {
            self.packets.pop_front();
        }
    }

    fn note(&mut self, now: i64, title: impl Into<String>, detail: impl Into<String>) {
        self.diag.push_back(DiagEntry { ts_ms: now, title: title.into(), detail: detail.into() });
        while self.diag.len() > DIAG_CAP {
            self.diag.pop_front();
        }
    }

    fn send(&mut self, now: i64, cmd: Command, out: &mut Vec<CoreEvent>) {
        let frame = cmd.to_frame();
        let bytes = frame.encode();
        self.log_packet(now, true, &bytes, frame.id);
        out.push(CoreEvent::Send { bytes });
    }

    fn state_changed(&self, out: &mut Vec<CoreEvent>) {
        out.push(CoreEvent::StateChanged { state: self.state.clone() });
    }

    // ---- incoming ----------------------------------------------------------

    fn feed(&mut self, now: i64, data: &[u8], out: &mut Vec<CoreEvent>) {
        for res in self.decoder.push(data) {
            match res {
                Ok(frame) => {
                    let raw = frame.encode();
                    self.log_packet(now, false, &raw, frame.id);
                    let msg = Incoming::parse(&frame);
                    self.handle(now, msg, out);
                }
                Err(p::FrameError::Skipped(n)) => {
                    self.note(now, "Protokol", format!("{n} bayt çöp veri atlandı"));
                }
                Err(e) => self.note(now, "Protokol hatası", format!("{e:?}")),
            }
        }
    }

    fn update_placement(&mut self, now: i64, l: Placement, r: Placement) {
        if self.state.placement_left != l || self.state.placement_right != r {
            if self.state.ready {
                self.last_placement_change_ms = now;
                self.note(
                    now,
                    "Takma durumu değişti",
                    format!("Sol: {:?} → {:?}, Sağ: {:?} → {:?}", self.state.placement_left, l, self.state.placement_right, r),
                );
            }
            self.state.placement_left = l;
            self.state.placement_right = r;
        }
    }

    fn update_battery(&mut self, now: i64, out: &mut Vec<CoreEvent>) {
        let (l, r, c) = (self.state.battery_left, self.state.battery_right, self.state.battery_case);
        // Low battery alerts (only while worn and not charging).
        let thr = self.config.low_battery_threshold;
        for (i, side, level, charging) in [
            (0usize, BudSide::Left, l, self.state.charging_left),
            (1, BudSide::Right, r, self.state.charging_right),
        ] {
            if let Some(level) = level {
                if level > 0 && level <= thr && !charging {
                    if !self.low_battery_fired[i] {
                        self.low_battery_fired[i] = true;
                        out.push(CoreEvent::LowBattery { side, level });
                    }
                } else if level > thr + 5 || charging {
                    self.low_battery_fired[i] = false;
                }
            }
        }
        let changed = match self.battery.last() {
            None => true,
            Some(last) => {
                (last.left != l || last.right != r || last.case_level != c) && now - last.ts_ms >= 60_000
                    || now - last.ts_ms >= BATTERY_MIN_INTERVAL_MS
            }
        };
        if changed && (l.is_some() || r.is_some()) {
            self.append_battery(BatterySample { ts_ms: now, left: l, right: r, case_level: c });
        }
    }

    fn handle(&mut self, now: i64, msg: Incoming, out: &mut Vec<CoreEvent>) {
        match msg {
            Incoming::ExtendedStatus(s) => {
                let first = !self.state.ready;
                let st = &mut self.state;
                st.status_revision = s.revision as i32;
                st.battery_left = Some(s.battery_left as i32);
                st.battery_right = Some(s.battery_right as i32);
                st.battery_case = s.battery_case.map(|x| x as i32);
                st.main_side = s.main_side.map(Into::into);
                st.eq = s.eq_preset.map(Into::into).or(st.eq);
                st.touch = s.gestures.map(Into::into).or(st.touch);
                st.hold_left = s.hold_left.map(Into::into).or(st.hold_left);
                st.hold_right = s.hold_right.map(Into::into).or(st.hold_right);
                st.noise_control = s.noise_mode.map(Into::into).or(st.noise_control);
                st.color = s.color_id.and_then(color_name).or(st.color.take());
                st.seamless_connection = s.seamless_connection.or(st.seamless_connection);
                st.cycle_left = s.noise_cycle_left.map(Into::into).or(st.cycle_left);
                st.cycle_right = s.noise_cycle_right.map(Into::into).or(st.cycle_right);
                st.ambient_volume = s.ambient_volume.map(|x| x as i32).or(st.ambient_volume);
                st.noise_reduction_high = s.noise_reduction_high.or(st.noise_reduction_high);
                st.stereo_balance = s.stereo_balance.map(|x| x as i32).or(st.stereo_balance);
                st.anc_with_one_earbud = s.anc_with_one_earbud.or(st.anc_with_one_earbud);
                st.outside_double_tap = s.outside_double_tap.or(st.outside_double_tap);
                st.sidetone = s.sidetone.or(st.sidetone);
                st.call_path_control = s.call_path_control.or(st.call_path_control);
                st.charging_left = s.charging_left.unwrap_or(false);
                st.charging_right = s.charging_right.unwrap_or(false);
                st.charging_case = s.charging_case.unwrap_or(false);
                self.update_placement(
                    now,
                    s.placement_left.map(Into::into).unwrap_or_default(),
                    s.placement_right.map(Into::into).unwrap_or_default(),
                );
                self.state.ready = true;
                if first {
                    self.note(now, "Bağlandı", format!("Durum revizyonu {}, {} bayt", s.revision, s.raw.len()));
                    self.enforce_touch(now, out);
                    self.apply_wifi_profile(now, out);
                }
                self.update_battery(now, out);
                self.state_changed(out);
            }
            Incoming::Status(s) => {
                let st = &mut self.state;
                st.battery_left = Some(s.battery_left as i32);
                st.battery_right = Some(s.battery_right as i32);
                st.battery_case = s.battery_case.map(|x| x as i32);
                st.main_side = Some(s.main_side.into());
                if let Some(c) = s.charging_left {
                    st.charging_left = c;
                }
                if let Some(c) = s.charging_right {
                    st.charging_right = c;
                }
                if let Some(c) = s.charging_case {
                    st.charging_case = c;
                }
                self.update_placement(now, s.placement_left.into(), s.placement_right.into());
                self.update_battery(now, out);
                self.state_changed(out);
            }
            Incoming::NoiseMode(m) => {
                self.state.noise_control = Some(m.into());
                self.state_changed(out);
            }
            Incoming::TouchLocked(locked) => {
                if let Some(t) = self.state.touch.as_mut() {
                    t.locked = locked;
                }
                self.note(now, "Dokunmatik", if locked { "Kilitlendi" } else { "Kilit açıldı" });
                self.state_changed(out);
            }
            Incoming::CustomHold(side) => self.on_custom_hold(now, side.into(), out),
            Incoming::FitResult { left, right } => {
                self.state.fit_left = Some(fit_name(left));
                self.state.fit_right = Some(fit_name(right));
                self.state_changed(out);
            }
            Incoming::MuteStatus { left, right } => {
                self.state.muted_left = left;
                self.state.muted_right = right;
                self.state_changed(out);
            }
            Incoming::Sku { left, right } => {
                let sku = left.filter(|s| !s.is_empty()).or(right.filter(|s| !s.is_empty()));
                if let Some(sku) = &sku {
                    if !sku.contains(p::MODEL_PREFIX) {
                        out.push(CoreEvent::Notice {
                            message: format!("Bu cihaz Buds Core (SM-R410) değil gibi görünüyor: {sku}. Bazı ayarlar yanlış çalışabilir."),
                        });
                    }
                }
                self.state.model_sku = sku;
                self.state_changed(out);
            }
            Incoming::UsageReport(items) => {
                for (k, v) in &items {
                    match k.as_str() {
                        "PAUS" => self.state.single_tap_pause_count = Some(*v as i64),
                        "PLAY" => self.state.single_tap_play_count = Some(*v as i64),
                        _ => {}
                    }
                }
                self.note(now, "Kullanım raporu", items.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(", "));
                self.state_changed(out);
            }
            Incoming::Ack { id, params } => {
                self.note(now, "Onay", format!("{} ← {}", ids::name(id), hex(&params)));
            }
            Incoming::Other { .. } => {}
        }
    }

    // ---- custom hold engine ------------------------------------------------

    fn on_custom_hold(&mut self, now: i64, side: BudSide, out: &mut Vec<CoreEvent>) {
        let window = self.config.custom_holds.multi_hold_window_ms;
        let count = match &mut self.pending_hold {
            Some(p) if p.side == side && now - p.last_ms <= window => {
                p.count += 1;
                p.last_ms = now;
                p.count
            }
            _ => {
                // A pending hold on the other side resolves first.
                self.flush_hold(now, true, out);
                self.pending_hold = Some(PendingHold { side, count: 1, last_ms: now });
                1
            }
        };
        if count >= self.config.custom_holds.max_count(side) {
            self.flush_hold(now, true, out);
        }
    }

    fn flush_hold(&mut self, now: i64, force: bool, out: &mut Vec<CoreEvent>) {
        let window = self.config.custom_holds.multi_hold_window_ms;
        let Some(ph) = &self.pending_hold else { return };
        if !force && now - ph.last_ms < window {
            return;
        }
        let (side, count) = (ph.side, ph.count);
        self.pending_hold = None;
        let action = self.config.custom_holds.for_hold(side, count);
        self.note(now, "Özel basılı tutma", format!("{side:?} ×{count} → {action:?}"));
        self.run_action(now, action, out);
    }

    fn run_action(&mut self, now: i64, action: AppAction, out: &mut Vec<CoreEvent>) {
        match action {
            AppAction::Nothing => {}
            AppAction::CycleNoiseControl => {
                let modes = if self.config.app_noise_cycle.is_valid() {
                    self.config.app_noise_cycle.modes()
                } else {
                    NoiseCycleSet::default().modes()
                };
                let cur = self.state.noise_control.unwrap_or(NoiseControl::Off);
                let next = match modes.iter().position(|m| *m == cur) {
                    Some(i) => modes[(i + 1) % modes.len()],
                    None => modes[0],
                };
                self.set_noise(now, next, out);
            }
            AppAction::ToggleAnc => {
                let next = if self.state.noise_control == Some(NoiseControl::Anc) { NoiseControl::Off } else { NoiseControl::Anc };
                self.set_noise(now, next, out);
            }
            AppAction::ToggleAmbient => {
                let next =
                    if self.state.noise_control == Some(NoiseControl::Ambient) { NoiseControl::Off } else { NoiseControl::Ambient };
                self.set_noise(now, next, out);
            }
            a => {
                if a == AppAction::PlayPause {
                    self.last_app_playpause_ms = now;
                }
                out.push(CoreEvent::RunAction { action: a });
            }
        }
    }

    fn set_noise(&mut self, now: i64, mode: NoiseControl, out: &mut Vec<CoreEvent>) {
        self.send(now, Command::SetNoiseMode(mode.into()), out);
        self.state.noise_control = Some(mode);
        self.state_changed(out);
    }

    fn enforce_touch(&mut self, now: i64, out: &mut Vec<CoreEvent>) {
        if !self.config.enforce_touch_settings || self.touch_enforced_this_connection {
            return;
        }
        let Some(saved) = self.config.saved_touch else { return };
        self.touch_enforced_this_connection = true;
        if self.state.touch != Some(saved) {
            self.send(now, Command::SetGestures(saved.into()), out);
            self.state.touch = Some(saved);
            self.note(now, "Dokunmatik", "Kayıtlı dokunma ayarları yeniden uygulandı");
        }
    }

    fn apply_wifi_profile(&mut self, now: i64, out: &mut Vec<CoreEvent>) {
        if !self.state.ready {
            return;
        }
        let Some(ssid) = self.current_ssid.clone() else { return };
        let Some(profile) = self.config.wifi_profiles.iter().find(|p| p.ssid == ssid).cloned() else { return };
        if let Some(nc) = profile.noise_control {
            if self.state.noise_control != Some(nc) {
                self.set_noise(now, nc, out);
            }
        }
        if let Some(eq) = profile.eq {
            if self.state.eq != Some(eq) {
                self.send(now, Command::SetEqualizer(eq.into()), out);
                self.state.eq = Some(eq);
                self.state_changed(out);
            }
        }
        self.note(now, "Wi-Fi profili", format!("\"{ssid}\" profili uygulandı"));
        out.push(CoreEvent::Notice { message: format!("\"{ssid}\" profili uygulandı") });
    }
}

macro_rules! with {
    ($self:ident, $now:ident, |$inner:ident, $out:ident| $body:block) => {{
        let mut $inner = $self.inner.lock().unwrap();
        let mut $out = Vec::new();
        let _ = $now;
        $body;
        $out
    }};
}

#[uniffi::export]
impl Session {
    /// `data_dir`: app private dir for config + battery history ("" = in-memory).
    #[uniffi::constructor]
    pub fn new(data_dir: String) -> Arc<Self> {
        let dir = if data_dir.is_empty() { None } else { Some(PathBuf::from(data_dir)) };
        Arc::new(Session { inner: Mutex::new(Inner::new(dir)) })
    }

    pub fn spp_uuid(&self) -> String {
        p::SPP_UUID.to_string()
    }

    // ---- connection lifecycle ---------------------------------------------

    pub fn on_connected(&self, now_ms: i64, android_sdk: i32, utc_offset_ms: i32) -> Vec<CoreEvent> {
        with!(self, now_ms, |s, out| {
            s.decoder.clear();
            s.state = DeviceState { connected: true, ..Default::default() };
            s.touch_enforced_this_connection = false;
            s.pending_hold = None;
            let client_type = s.config.manager_client_type.clamp(1, 2) as u8;
            s.send(now_ms, Command::ManagerInfo { client_type, android_sdk: android_sdk.clamp(0, 255) as u8 }, &mut out);
            s.send(now_ms, Command::UpdateTime { unix_ms: now_ms, utc_offset_ms }, &mut out);
            s.send(now_ms, Command::RequestSku, &mut out);
            s.note(now_ms, "Bağlantı", "RFCOMM kanalı açıldı");
            s.state_changed(&mut out);
        })
    }

    pub fn on_disconnected(&self, now_ms: i64, reason: String) -> Vec<CoreEvent> {
        with!(self, now_ms, |s, out| {
            s.last_disconnect_ms = now_ms;
            s.state.connected = false;
            s.state.ready = false;
            s.state.finding = false;
            s.decoder.clear();
            s.note(now_ms, "Bağlantı koptu", reason);
            s.state_changed(&mut out);
        })
    }

    /// Bytes received from the RFCOMM socket.
    pub fn feed(&self, data: Vec<u8>, now_ms: i64) -> Vec<CoreEvent> {
        with!(self, now_ms, |s, out| {
            s.feed(now_ms, &data, &mut out);
        })
    }

    /// Call ~every 250 ms while connected (resolves pending multi-holds).
    pub fn tick(&self, now_ms: i64) -> Vec<CoreEvent> {
        with!(self, now_ms, |s, out| {
            s.flush_hold(now_ms, false, &mut out);
        })
    }

    pub fn has_pending_hold(&self) -> bool {
        self.inner.lock().unwrap().pending_hold.is_some()
    }

    // ---- commands ----------------------------------------------------------

    pub fn set_noise_control(&self, mode: NoiseControl, now_ms: i64) -> Vec<CoreEvent> {
        with!(self, now_ms, |s, out| {
            s.set_noise(now_ms, mode, &mut out);
        })
    }

    pub fn set_ambient_volume(&self, level: i32, now_ms: i64) -> Vec<CoreEvent> {
        with!(self, now_ms, |s, out| {
            let l = level.clamp(0, 2);
            s.send(now_ms, Command::SetAmbientVolume(l as u8), &mut out);
            s.state.ambient_volume = Some(l);
            s.state_changed(&mut out);
        })
    }

    pub fn set_noise_reduction_high(&self, high: bool, now_ms: i64) -> Vec<CoreEvent> {
        with!(self, now_ms, |s, out| {
            s.send(now_ms, Command::SetNoiseReductionHigh(high), &mut out);
            s.state.noise_reduction_high = Some(high);
            s.state_changed(&mut out);
        })
    }

    pub fn set_eq(&self, preset: EqPreset, now_ms: i64) -> Vec<CoreEvent> {
        with!(self, now_ms, |s, out| {
            s.send(now_ms, Command::SetEqualizer(preset.into()), &mut out);
            s.state.eq = Some(preset);
            s.state_changed(&mut out);
        })
    }

    pub fn set_touch(&self, touch: TouchSettings, now_ms: i64) -> Vec<CoreEvent> {
        with!(self, now_ms, |s, out| {
            s.send(now_ms, Command::SetGestures(touch.into()), &mut out);
            s.state.touch = Some(touch);
            s.config.saved_touch = Some(touch);
            s.save_config();
            s.state_changed(&mut out);
        })
    }

    pub fn set_hold_modes(&self, left: HoldMode, right: HoldMode, now_ms: i64) -> Vec<CoreEvent> {
        with!(self, now_ms, |s, out| {
            s.send(now_ms, Command::SetHoldActions { left: left.into(), right: right.into() }, &mut out);
            s.state.hold_left = Some(left);
            s.state.hold_right = Some(right);
            s.state_changed(&mut out);
        })
    }

    pub fn set_noise_cycle(&self, left: NoiseCycleSet, right: NoiseCycleSet, now_ms: i64) -> Vec<CoreEvent> {
        with!(self, now_ms, |s, out| {
            if !left.is_valid() || !right.is_valid() {
                out.push(CoreEvent::Notice { message: "Her taraf için en az iki mod seçmelisin".into() });
            } else {
                s.send(now_ms, Command::SetNoiseCycle { left: left.into(), right: right.into() }, &mut out);
                s.state.cycle_left = Some(left);
                s.state.cycle_right = Some(right);
                s.state_changed(&mut out);
            }
        })
    }

    pub fn set_outside_double_tap(&self, on: bool, now_ms: i64) -> Vec<CoreEvent> {
        with!(self, now_ms, |s, out| {
            s.send(now_ms, Command::SetOutsideDoubleTap(on), &mut out);
            s.state.outside_double_tap = Some(on);
            s.state_changed(&mut out);
        })
    }

    pub fn find(&self, start: bool, now_ms: i64) -> Vec<CoreEvent> {
        with!(self, now_ms, |s, out| {
            s.send(now_ms, if start { Command::FindStart } else { Command::FindStop }, &mut out);
            s.state.finding = start;
            if !start {
                s.state.muted_left = false;
                s.state.muted_right = false;
            }
            s.state_changed(&mut out);
        })
    }

    pub fn mute_earbuds(&self, left: bool, right: bool, now_ms: i64) -> Vec<CoreEvent> {
        with!(self, now_ms, |s, out| {
            s.send(now_ms, Command::MuteEarbuds { left, right }, &mut out);
            s.state.muted_left = left;
            s.state.muted_right = right;
            s.state_changed(&mut out);
        })
    }

    pub fn fit_test(&self, start: bool, now_ms: i64) -> Vec<CoreEvent> {
        with!(self, now_ms, |s, out| {
            if start {
                s.state.fit_left = None;
                s.state.fit_right = None;
            }
            s.send(now_ms, Command::FitTest(start), &mut out);
            s.state_changed(&mut out);
        })
    }

    pub fn set_seamless_connection(&self, on: bool, now_ms: i64) -> Vec<CoreEvent> {
        with!(self, now_ms, |s, out| {
            s.send(now_ms, Command::SetSeamlessConnection(on), &mut out);
            s.state.seamless_connection = Some(on);
            s.state_changed(&mut out);
        })
    }

    pub fn set_sidetone(&self, on: bool, now_ms: i64) -> Vec<CoreEvent> {
        with!(self, now_ms, |s, out| {
            s.send(now_ms, Command::SetSidetone(on), &mut out);
            s.state.sidetone = Some(on);
            s.state_changed(&mut out);
        })
    }

    pub fn set_anc_with_one_earbud(&self, on: bool, now_ms: i64) -> Vec<CoreEvent> {
        with!(self, now_ms, |s, out| {
            s.send(now_ms, Command::SetAncWithOneEarbud(on), &mut out);
            s.state.anc_with_one_earbud = Some(on);
            s.state_changed(&mut out);
        })
    }

    pub fn set_call_path_control(&self, on: bool, now_ms: i64) -> Vec<CoreEvent> {
        with!(self, now_ms, |s, out| {
            s.send(now_ms, Command::SetCallPathControl(on), &mut out);
            s.state.call_path_control = Some(on);
            s.state_changed(&mut out);
        })
    }

    pub fn set_stereo_balance(&self, value: i32, now_ms: i64) -> Vec<CoreEvent> {
        with!(self, now_ms, |s, out| {
            let v = value.clamp(0, 32);
            s.send(now_ms, Command::SetStereoBalance(v as u8), &mut out);
            s.state.stereo_balance = Some(v);
            s.state_changed(&mut out);
        })
    }

    /// Debug: send an arbitrary message id with payload.
    pub fn send_raw(&self, id: i32, payload: Vec<u8>, now_ms: i64) -> Vec<CoreEvent> {
        with!(self, now_ms, |s, out| {
            s.send(now_ms, Command::Raw { id: id.clamp(0, 255) as u8, payload }, &mut out);
        })
    }

    /// One-tap fix for accidental pauses: turn off single tap in firmware and
    /// move play/pause to a custom touch-and-hold handled by BudsCrab.
    pub fn apply_tap_guard(&self, now_ms: i64) -> Vec<CoreEvent> {
        with!(self, now_ms, |s, out| {
            let mut touch = s.state.touch.unwrap_or_default();
            touch.locked = false;
            touch.single_tap = false;
            touch.hold = true;
            s.send(now_ms, Command::SetGestures(touch.into()), &mut out);
            s.send(
                now_ms,
                Command::SetHoldActions { left: p::HoldAction::Custom, right: p::HoldAction::Custom },
                &mut out,
            );
            s.state.touch = Some(touch);
            s.state.hold_left = Some(HoldMode::Custom);
            s.state.hold_right = Some(HoldMode::Custom);
            s.config.saved_touch = Some(touch);
            s.config.enforce_touch_settings = true;
            if s.config.custom_holds.right_hold == AppAction::Nothing {
                s.config.custom_holds.right_hold = AppAction::PlayPause;
            }
            s.save_config();
            s.note(now_ms, "Tek dokunma koruması", "Tek dokunma kapatıldı, oynat/duraklat basılı tutmaya taşındı");
            out.push(CoreEvent::Notice {
                message: "Tek dokunma kapandı. Oynat/duraklat: sağ kulaklığı basılı tut".into(),
            });
            s.state_changed(&mut out);
        })
    }

    /// Run an action from the UI / quick settings tile.
    pub fn perform(&self, action: AppAction, now_ms: i64) -> Vec<CoreEvent> {
        with!(self, now_ms, |s, out| {
            s.run_action(now_ms, action, &mut out);
        })
    }

    // ---- phone context -----------------------------------------------------

    pub fn on_wifi_changed(&self, ssid: Option<String>, now_ms: i64) -> Vec<CoreEvent> {
        with!(self, now_ms, |s, out| {
            let ssid = ssid.map(|x| x.trim_matches('"').to_string()).filter(|x| !x.is_empty() && x != "<unknown ssid>");
            if s.current_ssid != ssid {
                s.current_ssid = ssid;
                s.apply_wifi_profile(now_ms, &mut out);
            }
        })
    }

    /// Kotlin noticed media playback stopped. Records a guess why, so the user
    /// can see whether pauses come from touches, wear detection, or elsewhere.
    pub fn on_media_paused(&self, now_ms: i64) -> String {
        let mut s = self.inner.lock().unwrap();
        let st = &s.state;
        let reason = if !st.connected && now_ms - s.last_disconnect_ms < 3000 {
            "Bluetooth bağlantısı koptu".to_string()
        } else if now_ms - s.last_app_playpause_ms < 2000 {
            "BudsCrab basılı tutma aksiyonu (bilerek)".to_string()
        } else if now_ms - s.last_placement_change_ms < 2500 {
            "Kulaklık takma/çıkarma algısı (sensör)".to_string()
        } else if !st.connected {
            "Kulaklık bağlı değilken (telefon/uygulama)".to_string()
        } else if st.touch.map(|t| t.single_tap && !t.locked).unwrap_or(true) {
            "Büyük ihtimalle tek dokunma (tek dokunma açık)".to_string()
        } else {
            "Telefon veya uygulama kaynaklı (dokunma kapalı)".to_string()
        };
        s.note(now_ms, "Müzik durdu", reason.clone());
        reason
    }

    // ---- getters -----------------------------------------------------------

    pub fn state(&self) -> DeviceState {
        self.inner.lock().unwrap().state.clone()
    }

    pub fn config(&self) -> Config {
        self.inner.lock().unwrap().config.clone()
    }

    pub fn set_config(&self, config: Config) {
        let mut s = self.inner.lock().unwrap();
        s.config = config;
        s.save_config();
    }

    pub fn battery_history(&self, since_ms: i64) -> Vec<BatterySample> {
        self.inner.lock().unwrap().battery.iter().filter(|b| b.ts_ms >= since_ms).cloned().collect()
    }

    pub fn diagnostics(&self) -> Vec<DiagEntry> {
        self.inner.lock().unwrap().diag.iter().rev().cloned().collect()
    }

    pub fn packet_log(&self) -> Vec<PacketEntry> {
        self.inner.lock().unwrap().packets.iter().rev().cloned().collect()
    }

    pub fn clear_logs(&self) {
        let mut s = self.inner.lock().unwrap();
        s.packets.clear();
        s.diag.clear();
    }

    /// Raw EXTENDED_STATUS payload as hex, for verifying the Core layout.
    pub fn export_debug_report(&self) -> String {
        let s = self.inner.lock().unwrap();
        let mut r = String::new();
        r.push_str(&format!("BudsCrab {} debug raporu\n", env!("CARGO_PKG_VERSION")));
        r.push_str(&format!("SKU: {:?}\n\nDurum: {:#?}\n\nPaketler (eskiden yeniye):\n", s.state.model_sku, s.state));
        for p in &s.packets {
            r.push_str(&format!("{} {} {:>3} {:<34} {}\n", p.ts_ms, if p.outgoing { "TX" } else { "RX" }, p.id, p.name, p.hex));
        }
        r
    }
}

/// Decode a hex string of one or more frames (debug helper for the UI).
#[uniffi::export]
pub fn decode_hex(hex: String) -> String {
    let bytes: Vec<u8> = hex
        .split(|c: char| c.is_whitespace() || c == ',' || c == ':')
        .filter(|t| !t.is_empty())
        .filter_map(|t| u8::from_str_radix(t.trim_start_matches("0x"), 16).ok())
        .collect();
    let frames = p::scan(&bytes);
    if frames.is_empty() {
        return "Geçerli paket bulunamadı".into();
    }
    frames
        .iter()
        .map(|(off, f)| format!("@{off}: {} ({}) {:?}", ids::name(f.id), f.id, Incoming::parse(f)))
        .collect::<Vec<_>>()
        .join("\n")
}
