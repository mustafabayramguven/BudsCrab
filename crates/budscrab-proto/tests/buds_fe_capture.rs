//! Real EXTENDED_STATUS_UPDATED capture from a Buds FE (revision 2), taken from
//! GalaxyBudsClient's test data. The Buds Core uses the same layout.
use budscrab_proto::*;

const CAPTURE: &[u8] = include_bytes!("data/BudsFe_rev2.bin");

#[test]
fn decodes_real_extended_status() {
    let frame = Frame::decode(CAPTURE).expect("valid frame");
    assert_eq!(frame.id, ids::EXTENDED_STATUS_UPDATED);
    assert!(frame.response_flag);
    // Re-encoding gives byte identical output.
    assert_eq!(frame.encode(), CAPTURE);

    let Incoming::ExtendedStatus(s) = Incoming::parse(&frame) else { panic!("not ext status") };
    assert_eq!(s.revision, 2);
    assert_eq!((s.battery_left, s.battery_right), (58, 89));
    assert!(s.coupled);
    assert_eq!(s.main_side, Some(Side::Right));
    assert_eq!(s.placement_left, Some(Placement::Wearing));
    assert_eq!(s.placement_right, Some(Placement::Wearing));
    assert_eq!(s.eq_preset, Some(EqPreset::Off));
    let g = s.gestures.unwrap();
    assert!(!g.locked && g.single_tap && g.double_tap && g.triple_tap && g.hold);
    assert!(!g.double_tap_call && !g.hold_call);
    assert_eq!(s.hold_left, Some(HoldAction::Volume));
    assert_eq!(s.hold_right, Some(HoldAction::Volume));
    assert_eq!(s.noise_mode, Some(NoiseMode::Off));
    assert_eq!(s.color_id, Some(330)); // BudsFeGraphite
    assert_eq!(s.seamless_connection, Some(true));
    assert_eq!(s.noise_cycle_right, Some(NoiseCycle { off: true, ambient: false, anc: true }));
    assert_eq!(s.noise_cycle_left, Some(NoiseCycle { off: false, ambient: true, anc: true }));
    assert_eq!(s.ambient_volume, Some(1));
    assert_eq!(s.stereo_balance, Some(16));
    assert_eq!(s.anc_with_one_earbud, Some(true));
    assert_eq!(s.ambient_custom_enabled, Some(true));
    assert_eq!((s.ambient_custom_left, s.ambient_custom_right, s.ambient_custom_tone), (Some(1), Some(1), Some(3)));
    assert_eq!(s.outside_double_tap, Some(false));
    assert_eq!(s.sidetone, Some(false));
    assert_eq!(s.call_path_control, Some(true));
    assert_eq!((s.charging_left, s.charging_right, s.charging_case), (Some(false), Some(false), Some(false)));
}

#[test]
fn truncated_status_does_not_panic() {
    for n in 0..60 {
        let payload = vec![0x11u8; n];
        let f = Frame { id: ids::EXTENDED_STATUS_UPDATED, payload, response_flag: true, fragment: false };
        let _ = Incoming::parse(&f);
    }
}

#[test]
fn command_layouts() {
    let g = Gestures { single_tap: false, ..Default::default() };
    assert_eq!(Command::SetGestures(g).to_frame().payload, vec![1, 0, 1, 1, 1, 1, 1]);
    assert_eq!(
        Command::SetHoldActions { left: HoldAction::Custom, right: HoldAction::Custom }.to_frame().payload,
        vec![5, 6]
    );
    assert_eq!(
        Command::SetNoiseCycle { left: NoiseCycle { anc: true, ambient: false, off: true }, right: NoiseCycle::default() }
            .to_frame()
            .payload,
        vec![1, 0, 1, 1, 1, 0]
    );
    assert_eq!(Command::SetEqualizer(EqPreset::Dynamic).to_frame().payload, vec![3]);
    assert_eq!(Command::SetSeamlessConnection(true).to_frame().payload, vec![0]);
    assert_eq!(Command::UpdateTime { unix_ms: 1, utc_offset_ms: 10_800_000 }.to_frame().payload.len(), 12);
}
