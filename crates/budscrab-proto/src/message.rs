//! Incoming message decoders and outgoing command encoders for the Buds Core.
//!
//! The Buds Core shares the Buds FE layout (see GalaxyBudsClient's
//! `BudsCoreDeviceSpec`, which is identical to the FE one). Upstream notes the
//! Core layout is "probably the most similar" to the FE but not verified, so
//! every field read here is bounds checked and the raw payload is kept for
//! the debug screen.

use crate::frame::Frame;
use crate::ids::*;
use crate::model::*;

/// Decoded EXTENDED_STATUS_UPDATED (id 97) - the big "everything" message
/// the earbuds send right after the RFCOMM channel opens.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExtendedStatus {
    pub revision: u8,
    pub battery_left: u8,
    pub battery_right: u8,
    pub battery_case: Option<u8>,
    pub coupled: bool,
    /// Which bud holds the phone connection.
    pub main_side: Option<Side>,
    pub placement_left: Option<Placement>,
    pub placement_right: Option<Placement>,
    pub eq_preset: Option<EqPreset>,
    pub gestures: Option<Gestures>,
    pub hold_left: Option<HoldAction>,
    pub hold_right: Option<HoldAction>,
    pub noise_mode: Option<NoiseMode>,
    pub color_id: Option<u16>,
    pub seamless_connection: Option<bool>,
    pub noise_cycle_left: Option<NoiseCycle>,
    pub noise_cycle_right: Option<NoiseCycle>,
    pub ambient_volume: Option<u8>,
    pub noise_reduction_high: Option<bool>,
    /// Stereo balance / hearing enhancement byte (0..32, 16 = centered).
    pub stereo_balance: Option<u8>,
    pub anc_with_one_earbud: Option<bool>,
    pub ambient_custom_enabled: Option<bool>,
    pub ambient_custom_left: Option<u8>,
    pub ambient_custom_right: Option<u8>,
    pub ambient_custom_tone: Option<u8>,
    pub outside_double_tap: Option<bool>,
    pub sidetone: Option<bool>,
    pub call_path_control: Option<bool>,
    pub charging_left: Option<bool>,
    pub charging_right: Option<bool>,
    pub charging_case: Option<bool>,
    pub raw: Vec<u8>,
}

fn bit(b: u8, n: u8) -> bool {
    b & (1 << n) != 0
}

fn case_battery(b: u8) -> Option<u8> {
    // 0 is reported when the case battery is unknown / buds outside the case
    // on some firmwares; anything >100 is garbage.
    if b <= 100 { Some(b) } else { None }
}

impl ExtendedStatus {
    pub fn parse(p: &[u8]) -> Option<Self> {
        if p.len() < 8 {
            return None;
        }
        let g = |i: usize| p.get(i).copied();
        let mut s = ExtendedStatus {
            revision: p[0],
            battery_left: p[2],
            battery_right: p[3],
            coupled: p[4] != 0,
            main_side: Some(if p[5] == 1 { Side::Left } else { Side::Right }),
            placement_left: Some(Placement::from_nibble(p[6] >> 4)),
            placement_right: Some(Placement::from_nibble(p[6] & 0x0F)),
            battery_case: case_battery(p[7]),
            raw: p.to_vec(),
            ..Default::default()
        };
        s.eq_preset = g(9).and_then(EqPreset::from_byte);
        s.gestures = g(10).map(Gestures::from_status_byte);
        if let Some(b) = g(11) {
            s.hold_left = Some(HoldAction::from_byte(b >> 4));
            s.hold_right = Some(HoldAction::from_byte(b & 0x0F));
        }
        s.noise_mode = g(12).and_then(NoiseMode::from_byte);
        if let (Some(a), Some(b)) = (g(14), g(15)) {
            s.color_id = Some(u16::from_le_bytes([a, b]));
        }
        s.seamless_connection = g(19).map(|b| b == 0);
        if let Some(b) = g(21) {
            s.noise_cycle_right = Some(NoiseCycle { off: bit(b, 0), ambient: bit(b, 1), anc: bit(b, 2) });
            s.noise_cycle_left = Some(NoiseCycle { off: bit(b, 4), ambient: bit(b, 5), anc: bit(b, 6) });
        }
        s.ambient_volume = g(23);
        s.noise_reduction_high = g(24).map(|b| b != 0);
        s.stereo_balance = g(25);
        s.anc_with_one_earbud = g(28).map(|b| b == 1);
        s.ambient_custom_enabled = g(29).map(|b| b == 1);
        if let Some(b) = g(30) {
            s.ambient_custom_left = Some(b >> 4);
            s.ambient_custom_right = Some(b & 0x0F);
        }
        s.ambient_custom_tone = g(31);
        s.outside_double_tap = g(32).map(|b| b == 1);
        s.sidetone = g(33).map(|b| b == 1);
        s.call_path_control = g(34).map(|b| b == 0);
        if let Some(b) = g(43) {
            s.charging_left = Some(bit(b, 4));
            s.charging_right = Some(bit(b, 2));
            s.charging_case = Some(bit(b, 0));
        }
        Some(s)
    }
}

/// STATUS_UPDATED (id 96) - small periodic battery/placement update.
#[derive(Debug, Clone, PartialEq)]
pub struct Status {
    pub battery_left: u8,
    pub battery_right: u8,
    pub coupled: bool,
    pub main_side: Side,
    pub placement_left: Placement,
    pub placement_right: Placement,
    pub battery_case: Option<u8>,
    pub charging_left: Option<bool>,
    pub charging_right: Option<bool>,
    pub charging_case: Option<bool>,
}

impl Status {
    pub fn parse(p: &[u8]) -> Option<Self> {
        if p.len() < 7 {
            return None;
        }
        let charging = p.get(7).copied();
        Some(Status {
            battery_left: p[1],
            battery_right: p[2],
            coupled: p[3] != 0,
            main_side: if p[4] == 1 { Side::Left } else { Side::Right },
            placement_left: Placement::from_nibble(p[5] >> 4),
            placement_right: Placement::from_nibble(p[5] & 0x0F),
            battery_case: case_battery(p[6]),
            charging_left: charging.map(|b| bit(b, 4)),
            charging_right: charging.map(|b| bit(b, 2)),
            charging_case: charging.map(|b| bit(b, 0)),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Incoming {
    ExtendedStatus(ExtendedStatus),
    Status(Status),
    NoiseMode(NoiseMode),
    /// Firmware toggled the touchpad lock (e.g. by the user).
    TouchLocked(bool),
    /// Touch-and-hold on a bud whose hold action is `Custom`.
    CustomHold(Side),
    FitResult { left: FitResult, right: FitResult },
    MuteStatus { left: bool, right: bool },
    Ack { id: u8, params: Vec<u8> },
    Sku { left: Option<String>, right: Option<String> },
    UsageReport(Vec<(String, u32)>),
    Other { id: u8, payload: Vec<u8> },
}

impl Incoming {
    pub fn parse(f: &Frame) -> Incoming {
        let p = &f.payload;
        let other = || Incoming::Other { id: f.id, payload: p.clone() };
        match f.id {
            EXTENDED_STATUS_UPDATED => ExtendedStatus::parse(p).map(Incoming::ExtendedStatus).unwrap_or_else(other),
            STATUS_UPDATED => Status::parse(p).map(Incoming::Status).unwrap_or_else(other),
            NOISE_CONTROLS_UPDATE => p
                .first()
                .and_then(|b| NoiseMode::from_byte(*b))
                .map(Incoming::NoiseMode)
                .unwrap_or_else(other),
            TOUCH_UPDATED => p.first().map(|b| Incoming::TouchLocked(*b != 0)).unwrap_or_else(other),
            SET_TOUCHPAD_OTHER_OPTION => match p.first() {
                Some(5) => Incoming::CustomHold(Side::Left),
                Some(6) => Incoming::CustomHold(Side::Right),
                _ => other(),
            },
            CHECK_THE_FIT_OF_EARBUDS_RESULT if p.len() >= 2 => Incoming::FitResult {
                left: FitResult::from_byte(p[0]),
                right: FitResult::from_byte(p[1]),
            },
            MUTE_EARBUD_STATUS_UPDATED if p.len() >= 2 => Incoming::MuteStatus { left: p[0] != 0, right: p[1] != 0 },
            ACK if !p.is_empty() => Incoming::Ack { id: p[0], params: p[1..].to_vec() },
            DEBUG_SKU => {
                let txt = |r: std::ops::Range<usize>| {
                    p.get(r).map(|b| String::from_utf8_lossy(b).trim_matches(char::from(0)).trim().to_string())
                };
                Incoming::Sku { left: txt(0..14), right: txt(14..28) }
            }
            USAGE_REPORT => parse_usage(p).map(Incoming::UsageReport).unwrap_or_else(other),
            _ => other(),
        }
    }
}

fn parse_usage(p: &[u8]) -> Option<Vec<(String, u32)>> {
    let n = *p.first()? as usize;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let off = 1 + i * 9;
        let item = p.get(off..off + 9)?;
        let key: String = item[..5].iter().take_while(|b| **b != 0).map(|b| *b as char).collect();
        let val = u32::from_le_bytes([item[5], item[6], item[7], item[8]]);
        out.push((key, val));
    }
    Some(out)
}

/// Everything BudsCrab can ask the earbuds to do.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// Must be sent once after connecting; identifies us as the manager app.
    /// `client_type`: 1 = Samsung phone (what GalaxyBudsClient sends), 2 = other.
    ManagerInfo { client_type: u8, android_sdk: u8 },
    UpdateTime { unix_ms: i64, utc_offset_ms: i32 },
    RequestSku,
    SetNoiseMode(NoiseMode),
    SetAmbientVolume(u8),
    SetNoiseReductionHigh(bool),
    SetEqualizer(EqPreset),
    SetGestures(Gestures),
    SetHoldActions { left: HoldAction, right: HoldAction },
    SetNoiseCycle { left: NoiseCycle, right: NoiseCycle },
    SetOutsideDoubleTap(bool),
    FindStart,
    FindStop,
    MuteEarbuds { left: bool, right: bool },
    FitTest(bool),
    SetSeamlessConnection(bool),
    SetSidetone(bool),
    SetAncWithOneEarbud(bool),
    SetCallPathControl(bool),
    SetStereoBalance(u8),
    CustomizeAmbient { enabled: bool, left: u8, right: u8, tone: u8 },
    Raw { id: u8, payload: Vec<u8> },
}

fn b(v: bool) -> u8 {
    v as u8
}

impl Command {
    pub fn to_frame(&self) -> Frame {
        use Command::*;
        let (id, payload): (u8, Vec<u8>) = match self.clone() {
            ManagerInfo { client_type, android_sdk } => (MANAGER_INFO, vec![1, client_type, android_sdk]),
            UpdateTime { unix_ms, utc_offset_ms } => {
                let mut v = unix_ms.to_le_bytes().to_vec();
                v.extend_from_slice(&utc_offset_ms.to_le_bytes());
                (UPDATE_TIME, v)
            }
            RequestSku => (DEBUG_SKU, vec![]),
            SetNoiseMode(m) => (NOISE_CONTROLS, vec![m.to_byte()]),
            SetAmbientVolume(v) => (AMBIENT_VOLUME, vec![v.min(2)]),
            SetNoiseReductionHigh(v) => (NOISE_REDUCTION_LEVEL, vec![b(v)]),
            SetEqualizer(p) => (EQUALIZER, vec![p.to_byte()]),
            SetGestures(g) => (
                LOCK_TOUCHPAD,
                vec![b(!g.locked), b(g.single_tap), b(g.double_tap), b(g.triple_tap), b(g.hold), b(g.double_tap_call), b(g.hold_call)],
            ),
            SetHoldActions { left, right } => (SET_TOUCHPAD_OPTION, vec![left.to_byte(Side::Left), right.to_byte(Side::Right)]),
            SetNoiseCycle { left, right } => (
                SET_TOUCH_AND_HOLD_NOISE_CONTROLS,
                vec![b(left.anc), b(left.ambient), b(left.off), b(right.anc), b(right.ambient), b(right.off)],
            ),
            SetOutsideDoubleTap(v) => (OUTSIDE_DOUBLE_TAP, vec![b(v)]),
            FindStart => (FIND_MY_EARBUDS_START, vec![]),
            FindStop => (FIND_MY_EARBUDS_STOP, vec![]),
            MuteEarbuds { left, right } => (MUTE_EARBUD, vec![b(left), b(right)]),
            FitTest(v) => (CHECK_THE_FIT_OF_EARBUDS, vec![b(v)]),
            // Inverted on the wire: 0 = enabled
            SetSeamlessConnection(v) => (SET_SEAMLESS_CONNECTION, vec![b(!v)]),
            SetSidetone(v) => (SET_SIDETONE, vec![b(v)]),
            SetAncWithOneEarbud(v) => (SET_ANC_WITH_ONE_EARBUD, vec![b(v)]),
            SetCallPathControl(v) => (SET_CALL_PATH_CONTROL, vec![b(!v)]),
            SetStereoBalance(v) => (SET_HEARING_ENHANCEMENTS, vec![v.min(32)]),
            CustomizeAmbient { enabled, left, right, tone } => (CUSTOMIZE_AMBIENT_SOUND, vec![b(enabled), left, right, tone]),
            Raw { id, payload } => (id, payload),
        };
        Frame::request(id, payload)
    }

    pub fn encode(&self) -> Vec<u8> {
        self.to_frame().encode()
    }
}
