use crate::*;
use budscrab_proto::{ids, Frame, FrameDecoder};

const CAPTURE: &[u8] = include_bytes!("../../budscrab-proto/tests/data/BudsFe_rev2.bin");

fn sent(events: &[CoreEvent]) -> Vec<Frame> {
    let mut d = FrameDecoder::default();
    events
        .iter()
        .filter_map(|e| match e {
            CoreEvent::Send { bytes } => Some(bytes.clone()),
            _ => None,
        })
        .flat_map(|b| d.push(&b).into_iter().filter_map(Result::ok).collect::<Vec<_>>())
        .collect()
}

fn actions(events: &[CoreEvent]) -> Vec<AppAction> {
    events
        .iter()
        .filter_map(|e| match e {
            CoreEvent::RunAction { action } => Some(*action),
            _ => None,
        })
        .collect()
}

fn hold(side: u8) -> Vec<u8> {
    let mut f = Frame::request(ids::SET_TOUCHPAD_OTHER_OPTION, vec![side]);
    f.response_flag = true;
    f.encode()
}

fn connected() -> std::sync::Arc<Session> {
    let s = Session::new(String::new());
    let ev = s.on_connected(0, 35, 10_800_000);
    let ids_sent: Vec<u8> = sent(&ev).iter().map(|f| f.id).collect();
    assert_eq!(ids_sent, vec![ids::MANAGER_INFO, ids::UPDATE_TIME, ids::DEBUG_SKU]);
    s.feed(CAPTURE.to_vec(), 10);
    s
}

#[test]
fn connect_and_read_status() {
    let s = connected();
    let st = s.state();
    assert!(st.connected && st.ready);
    assert_eq!(st.battery_left, Some(58));
    assert_eq!(st.battery_right, Some(89));
    assert_eq!(st.placement_left, Placement::Wearing);
    assert_eq!(st.noise_control, Some(NoiseControl::Off));
    assert_eq!(st.touch.unwrap().single_tap, true);
    assert_eq!(st.hold_right, Some(HoldMode::Volume));
    assert_eq!(s.packet_log().len(), 4);
}

#[test]
fn split_feed_works() {
    let s = Session::new(String::new());
    s.on_connected(0, 35, 0);
    for (i, c) in CAPTURE.chunks(5).enumerate() {
        s.feed(c.to_vec(), i as i64);
    }
    assert!(s.state().ready);
}

#[test]
fn tap_guard_disables_single_tap_and_uses_custom_hold() {
    let s = connected();
    let ev = s.apply_tap_guard(100);
    let frames = sent(&ev);
    assert_eq!(frames[0].id, ids::LOCK_TOUCHPAD);
    assert_eq!(frames[0].payload, vec![1, 0, 1, 1, 1, 0, 0]);
    assert_eq!(frames[1].id, ids::SET_TOUCHPAD_OPTION);
    assert_eq!(frames[1].payload, vec![5, 6]);
    let st = s.state();
    assert!(!st.touch.unwrap().single_tap);
    assert_eq!(st.hold_left, Some(HoldMode::Custom));
    assert_eq!(s.config().saved_touch.unwrap().single_tap, false);
}

#[test]
fn single_and_double_hold() {
    let s = connected();
    // right: single = PlayPause, double = Next (default) -> must wait window
    let ev = s.feed(hold(6), 1000);
    assert!(actions(&ev).is_empty());
    assert!(s.has_pending_hold());
    assert!(actions(&s.tick(1500)).is_empty());
    assert_eq!(actions(&s.tick(2300)), vec![AppAction::PlayPause]);

    s.feed(hold(6), 5000);
    let ev = s.feed(hold(6), 5800);
    assert_eq!(actions(&ev), vec![AppAction::NextTrack]); // max count reached -> immediate
    assert!(!s.has_pending_hold());
}

#[test]
fn single_hold_is_immediate_when_no_multi_hold_configured() {
    let s = connected();
    let mut c = s.config();
    c.custom_holds.right_double_hold = AppAction::Nothing;
    c.custom_holds.right_triple_hold = AppAction::Nothing;
    s.set_config(c);
    assert_eq!(actions(&s.feed(hold(6), 1000)), vec![AppAction::PlayPause]);
}

#[test]
fn left_hold_cycles_noise_in_core() {
    let s = connected();
    s.feed(hold(5), 1000);
    let ev = s.tick(3000);
    let f = sent(&ev);
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].id, ids::NOISE_CONTROLS);
    assert_eq!(f[0].payload, vec![1]); // Off -> ANC
    assert_eq!(s.state().noise_control, Some(NoiseControl::Anc));
    assert_eq!(sent(&s.perform(AppAction::CycleNoiseControl, 4000))[0].payload, vec![2]); // ANC -> Ambient
}

#[test]
fn wifi_profile_applies() {
    let s = connected();
    let mut c = s.config();
    c.wifi_profiles.push(WifiProfile { ssid: "Yurt".into(), noise_control: Some(NoiseControl::Anc), eq: Some(EqPreset::Clear) });
    s.set_config(c);
    let ev = s.on_wifi_changed(Some("\"Yurt\"".into()), 100);
    let ids_sent: Vec<u8> = sent(&ev).iter().map(|f| f.id).collect();
    assert_eq!(ids_sent, vec![ids::NOISE_CONTROLS, ids::EQUALIZER]);
    // same ssid again -> nothing
    assert!(sent(&s.on_wifi_changed(Some("Yurt".into()), 200)).is_empty());
}

#[test]
fn pause_classification() {
    let s = connected();
    assert!(s.on_media_paused(60_000).contains("tek dokunma"));
    s.apply_tap_guard(61_000);
    assert!(s.on_media_paused(70_000).contains("Telefon"));
    s.feed(hold(6), 80_000);
    s.tick(82_000);
    assert!(s.on_media_paused(82_500).contains("BudsCrab"));
}

#[test]
fn low_battery_fires_once() {
    let s = connected();
    let mut c = s.config();
    c.low_battery_threshold = 60;
    s.set_config(c);
    // status: rev, L=50, R=90, coupled, main, placement, case, charging
    let mut f = Frame::request(ids::STATUS_UPDATED, vec![1, 50, 90, 1, 0, 0x11, 80, 0]);
    f.response_flag = true;
    let ev = s.feed(f.encode(), 100);
    assert!(ev.iter().any(|e| matches!(e, CoreEvent::LowBattery { side: BudSide::Left, level: 50 })));
    let ev = s.feed(f.encode(), 200);
    assert!(!ev.iter().any(|e| matches!(e, CoreEvent::LowBattery { .. })));
}

#[test]
fn persistence_roundtrip() {
    let dir = std::env::temp_dir().join(format!("budscrab-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    {
        let s = Session::new(dir.to_string_lossy().into());
        s.on_connected(0, 35, 0);
        s.feed(CAPTURE.to_vec(), 1);
        s.apply_tap_guard(2);
    }
    let s = Session::new(dir.to_string_lossy().into());
    assert_eq!(s.config().saved_touch.unwrap().single_tap, false);
    assert_eq!(s.battery_history(0).len(), 1);
    // reconnect -> buds still report single tap on -> core re-applies saved touch
    let ev = s.on_connected(10, 35, 0);
    assert!(!ev.is_empty());
    let ev = s.feed(CAPTURE.to_vec(), 11);
    assert!(sent(&ev).iter().any(|f| f.id == ids::LOCK_TOUCHPAD));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn decode_hex_helper() {
    let hex: String = CAPTURE.iter().map(|b| format!("{b:02x} ")).collect();
    assert!(decode_hex(hex).contains("EXTENDED_STATUS_UPDATED"));
}
