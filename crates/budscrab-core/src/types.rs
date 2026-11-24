//! FFI-facing types (exported to Kotlin through UniFFI) and conversions to the
//! protocol crate's types.

use budscrab_proto as p;
use serde::{Deserialize, Serialize};

#[derive(uniffi::Enum, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BudSide {
    Left,
    Right,
}

#[derive(uniffi::Enum, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoiseControl {
    Off,
    Anc,
    Ambient,
}

#[derive(uniffi::Enum, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum EqPreset {
    Off,
    BassBoost,
    Soft,
    Dynamic,
    Clear,
    TrebleBoost,
}

#[derive(uniffi::Enum, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Placement {
    #[default]
    Unknown,
    Disconnected,
    Wearing,
    Idle,
    InCase,
    ClosedCase,
}

/// Firmware-side touch-and-hold behaviour.
#[derive(uniffi::Enum, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoldMode {
    VoiceAssistant,
    NoiseControl,
    Volume,
    Spotify,
    /// Handled by BudsCrab (see [`AppAction`]).
    Custom,
    Unknown,
}

/// Actions BudsCrab can run when a bud reports a custom hold.
#[derive(uniffi::Enum, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppAction {
    Nothing,
    PlayPause,
    NextTrack,
    PreviousTrack,
    VolumeUp,
    VolumeDown,
    CycleNoiseControl,
    ToggleAnc,
    ToggleAmbient,
    VoiceAssistant,
}

impl AppAction {
    /// Actions the core can run on its own (by sending a command to the buds).
    pub fn handled_by_core(self) -> bool {
        matches!(self, AppAction::CycleNoiseControl | AppAction::ToggleAnc | AppAction::ToggleAmbient)
    }
}

#[derive(uniffi::Record, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct TouchSettings {
    pub locked: bool,
    pub single_tap: bool,
    pub double_tap: bool,
    pub triple_tap: bool,
    pub hold: bool,
    pub double_tap_call: bool,
    pub hold_call: bool,
}

impl Default for TouchSettings {
    fn default() -> Self {
        p::Gestures::default().into()
    }
}

#[derive(uniffi::Record, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoiseCycleSet {
    pub anc: bool,
    pub ambient: bool,
    pub off: bool,
}

impl Default for NoiseCycleSet {
    fn default() -> Self {
        NoiseCycleSet { anc: true, ambient: true, off: false }
    }
}

impl NoiseCycleSet {
    pub fn is_valid(&self) -> bool {
        [self.anc, self.ambient, self.off].iter().filter(|x| **x).count() >= 2
    }
    pub fn modes(&self) -> Vec<NoiseControl> {
        let mut v = Vec::new();
        if self.anc {
            v.push(NoiseControl::Anc);
        }
        if self.ambient {
            v.push(NoiseControl::Ambient);
        }
        if self.off {
            v.push(NoiseControl::Off);
        }
        v
    }
}

/// What to do on one, two or three holds in a row on each bud.
#[derive(uniffi::Record, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct CustomHoldActions {
    pub left_hold: AppAction,
    pub left_double_hold: AppAction,
    pub left_triple_hold: AppAction,
    pub right_hold: AppAction,
    pub right_double_hold: AppAction,
    pub right_triple_hold: AppAction,
    /// Time window to wait for another hold before acting.
    pub multi_hold_window_ms: i64,
}

impl Default for CustomHoldActions {
    fn default() -> Self {
        CustomHoldActions {
            left_hold: AppAction::CycleNoiseControl,
            left_double_hold: AppAction::PreviousTrack,
            left_triple_hold: AppAction::Nothing,
            right_hold: AppAction::PlayPause,
            right_double_hold: AppAction::NextTrack,
            right_triple_hold: AppAction::Nothing,
            multi_hold_window_ms: 1200,
        }
    }
}

impl CustomHoldActions {
    pub fn for_hold(&self, side: BudSide, count: u32) -> AppAction {
        match (side, count) {
            (BudSide::Left, 1) => self.left_hold,
            (BudSide::Left, 2) => self.left_double_hold,
            (BudSide::Left, _) => self.left_triple_hold,
            (BudSide::Right, 1) => self.right_hold,
            (BudSide::Right, 2) => self.right_double_hold,
            (BudSide::Right, _) => self.right_triple_hold,
        }
    }
    /// Highest hold count that does something on this side. 1 means we can act
    /// immediately without waiting for the multi-hold window.
    pub fn max_count(&self, side: BudSide) -> u32 {
        let (a, b) = match side {
            BudSide::Left => (self.left_double_hold, self.left_triple_hold),
            BudSide::Right => (self.right_double_hold, self.right_triple_hold),
        };
        if b != AppAction::Nothing {
            3
        } else if a != AppAction::Nothing {
            2
        } else {
            1
        }
    }
}

#[derive(uniffi::Record, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct WifiProfile {
    pub ssid: String,
    pub noise_control: Option<NoiseControl>,
    pub eq: Option<EqPreset>,
}

#[derive(uniffi::Record, Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(default)]
pub struct Config {
    pub custom_holds: CustomHoldActions,
    /// Modes the app-side "cycle noise control" action walks through.
    pub app_noise_cycle: NoiseCycleSet,
    pub wifi_profiles: Vec<WifiProfile>,
    pub low_battery_threshold: i32,
    /// Remember the touch settings and push them again on every connect, in
    /// case another app (or a reset) changed them.
    pub enforce_touch_settings: bool,
    pub saved_touch: Option<TouchSettings>,
    /// MANAGER_INFO client type: 1 = Samsung phone, 2 = other.
    pub manager_client_type: i32,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            custom_holds: CustomHoldActions::default(),
            app_noise_cycle: NoiseCycleSet::default(),
            wifi_profiles: vec![],
            low_battery_threshold: 15,
            enforce_touch_settings: true,
            saved_touch: None,
            manager_client_type: 1,
        }
    }
}

#[derive(uniffi::Record, Debug, Clone, PartialEq, Default)]
pub struct DeviceState {
    pub connected: bool,
    /// True once the first EXTENDED_STATUS_UPDATED arrived.
    pub ready: bool,
    pub model_sku: Option<String>,
    pub status_revision: i32,
    pub battery_left: Option<i32>,
    pub battery_right: Option<i32>,
    pub battery_case: Option<i32>,
    pub charging_left: bool,
    pub charging_right: bool,
    pub charging_case: bool,
    pub placement_left: Placement,
    pub placement_right: Placement,
    pub main_side: Option<BudSide>,
    pub noise_control: Option<NoiseControl>,
    pub ambient_volume: Option<i32>,
    pub noise_reduction_high: Option<bool>,
    pub eq: Option<EqPreset>,
    pub touch: Option<TouchSettings>,
    pub hold_left: Option<HoldMode>,
    pub hold_right: Option<HoldMode>,
    pub cycle_left: Option<NoiseCycleSet>,
    pub cycle_right: Option<NoiseCycleSet>,
    pub outside_double_tap: Option<bool>,
    pub seamless_connection: Option<bool>,
    pub sidetone: Option<bool>,
    pub anc_with_one_earbud: Option<bool>,
    pub call_path_control: Option<bool>,
    pub stereo_balance: Option<i32>,
    pub color: Option<String>,
    pub muted_left: bool,
    pub muted_right: bool,
    pub finding: bool,
    pub fit_left: Option<String>,
    pub fit_right: Option<String>,
    pub single_tap_pause_count: Option<i64>,
    pub single_tap_play_count: Option<i64>,
}

#[derive(uniffi::Record, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct BatterySample {
    pub ts_ms: i64,
    pub left: Option<i32>,
    pub right: Option<i32>,
    pub case_level: Option<i32>,
}

#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct DiagEntry {
    pub ts_ms: i64,
    pub title: String,
    pub detail: String,
}

#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct PacketEntry {
    pub ts_ms: i64,
    pub outgoing: bool,
    pub id: i32,
    pub name: String,
    pub hex: String,
}

#[derive(uniffi::Enum, Debug, Clone, PartialEq)]
pub enum CoreEvent {
    /// Write these bytes to the RFCOMM socket.
    Send { bytes: Vec<u8> },
    StateChanged { state: DeviceState },
    /// Run a phone-side action (media key, volume, assistant).
    RunAction { action: AppAction },
    LowBattery { side: BudSide, level: i32 },
    /// Human readable note for the UI (toast/snackbar).
    Notice { message: String },
}

// ---- conversions ---------------------------------------------------------

impl From<p::Side> for BudSide {
    fn from(s: p::Side) -> Self {
        match s {
            p::Side::Left => BudSide::Left,
            p::Side::Right => BudSide::Right,
        }
    }
}

impl From<p::NoiseMode> for NoiseControl {
    fn from(m: p::NoiseMode) -> Self {
        match m {
            p::NoiseMode::Off | p::NoiseMode::Adaptive => NoiseControl::Off,
            p::NoiseMode::Anc => NoiseControl::Anc,
            p::NoiseMode::Ambient => NoiseControl::Ambient,
        }
    }
}

impl From<NoiseControl> for p::NoiseMode {
    fn from(m: NoiseControl) -> Self {
        match m {
            NoiseControl::Off => p::NoiseMode::Off,
            NoiseControl::Anc => p::NoiseMode::Anc,
            NoiseControl::Ambient => p::NoiseMode::Ambient,
        }
    }
}

impl From<p::EqPreset> for EqPreset {
    fn from(e: p::EqPreset) -> Self {
        match e {
            p::EqPreset::Off => EqPreset::Off,
            p::EqPreset::BassBoost => EqPreset::BassBoost,
            p::EqPreset::Soft => EqPreset::Soft,
            p::EqPreset::Dynamic => EqPreset::Dynamic,
            p::EqPreset::Clear => EqPreset::Clear,
            p::EqPreset::TrebleBoost => EqPreset::TrebleBoost,
        }
    }
}

impl From<EqPreset> for p::EqPreset {
    fn from(e: EqPreset) -> Self {
        match e {
            EqPreset::Off => p::EqPreset::Off,
            EqPreset::BassBoost => p::EqPreset::BassBoost,
            EqPreset::Soft => p::EqPreset::Soft,
            EqPreset::Dynamic => p::EqPreset::Dynamic,
            EqPreset::Clear => p::EqPreset::Clear,
            EqPreset::TrebleBoost => p::EqPreset::TrebleBoost,
        }
    }
}

impl From<p::Placement> for Placement {
    fn from(pl: p::Placement) -> Self {
        match pl {
            p::Placement::Disconnected => Placement::Disconnected,
            p::Placement::Wearing => Placement::Wearing,
            p::Placement::Idle => Placement::Idle,
            p::Placement::InCase => Placement::InCase,
            p::Placement::ClosedCase => Placement::ClosedCase,
            p::Placement::Unknown(_) => Placement::Unknown,
        }
    }
}

impl From<p::HoldAction> for HoldMode {
    fn from(h: p::HoldAction) -> Self {
        match h {
            p::HoldAction::VoiceAssistant => HoldMode::VoiceAssistant,
            p::HoldAction::NoiseControl => HoldMode::NoiseControl,
            p::HoldAction::Volume => HoldMode::Volume,
            p::HoldAction::Spotify => HoldMode::Spotify,
            p::HoldAction::Custom => HoldMode::Custom,
            p::HoldAction::Unknown(_) => HoldMode::Unknown,
        }
    }
}

impl From<HoldMode> for p::HoldAction {
    fn from(h: HoldMode) -> Self {
        match h {
            HoldMode::VoiceAssistant => p::HoldAction::VoiceAssistant,
            HoldMode::NoiseControl => p::HoldAction::NoiseControl,
            HoldMode::Volume => p::HoldAction::Volume,
            HoldMode::Spotify => p::HoldAction::Spotify,
            HoldMode::Custom | HoldMode::Unknown => p::HoldAction::Custom,
        }
    }
}

impl From<p::Gestures> for TouchSettings {
    fn from(g: p::Gestures) -> Self {
        TouchSettings {
            locked: g.locked,
            single_tap: g.single_tap,
            double_tap: g.double_tap,
            triple_tap: g.triple_tap,
            hold: g.hold,
            double_tap_call: g.double_tap_call,
            hold_call: g.hold_call,
        }
    }
}

impl From<TouchSettings> for p::Gestures {
    fn from(t: TouchSettings) -> Self {
        p::Gestures {
            locked: t.locked,
            single_tap: t.single_tap,
            double_tap: t.double_tap,
            triple_tap: t.triple_tap,
            hold: t.hold,
            double_tap_call: t.double_tap_call,
            hold_call: t.hold_call,
        }
    }
}

impl From<p::NoiseCycle> for NoiseCycleSet {
    fn from(c: p::NoiseCycle) -> Self {
        NoiseCycleSet { anc: c.anc, ambient: c.ambient, off: c.off }
    }
}

impl From<NoiseCycleSet> for p::NoiseCycle {
    fn from(c: NoiseCycleSet) -> Self {
        p::NoiseCycle { anc: c.anc, ambient: c.ambient, off: c.off }
    }
}

pub fn color_name(id: u16) -> Option<String> {
    Some(
        match id {
            322 => "Onyx",
            323 => "White",
            330 => "Graphite (FE)",
            331 => "White (FE)",
            _ => return None,
        }
        .to_string(),
    )
}

pub fn fit_name(r: p::FitResult) -> String {
    match r {
        p::FitResult::Good => "İyi",
        p::FitResult::Bad => "Kötü",
        p::FitResult::Failed => "Test başarısız",
        p::FitResult::Unknown(_) => "Bilinmiyor",
    }
    .to_string()
}
