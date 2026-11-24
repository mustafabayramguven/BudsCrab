//! Plain data types shared by decoders and encoders.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoiseMode {
    Off,
    Anc,
    Ambient,
    Adaptive,
}

impl NoiseMode {
    pub fn from_byte(b: u8) -> Option<Self> {
        Some(match b {
            0 => NoiseMode::Off,
            1 => NoiseMode::Anc,
            2 => NoiseMode::Ambient,
            3 => NoiseMode::Adaptive,
            _ => return None,
        })
    }
    pub fn to_byte(self) -> u8 {
        match self {
            NoiseMode::Off => 0,
            NoiseMode::Anc => 1,
            NoiseMode::Ambient => 2,
            NoiseMode::Adaptive => 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    Disconnected,
    Wearing,
    Idle,
    InCase,
    ClosedCase,
    Unknown(u8),
}

impl Placement {
    pub fn from_nibble(n: u8) -> Self {
        match n {
            0 => Placement::Disconnected,
            1 => Placement::Wearing,
            2 => Placement::Idle,
            3 => Placement::InCase,
            4 => Placement::ClosedCase,
            x => Placement::Unknown(x),
        }
    }
}

/// What the earbud does on touch-and-hold (firmware-side option).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoldAction {
    VoiceAssistant,
    NoiseControl,
    Volume,
    Spotify,
    /// Firmware does nothing itself and reports SET_TOUCHPAD_OTHER_OPTION to
    /// the phone; BudsCrab then runs the user's custom action.
    Custom,
    Unknown(u8),
}

impl HoldAction {
    pub fn from_byte(b: u8) -> Self {
        match b {
            1 => HoldAction::VoiceAssistant,
            2 => HoldAction::NoiseControl,
            3 => HoldAction::Volume,
            4 => HoldAction::Spotify,
            5 | 6 => HoldAction::Custom,
            x => HoldAction::Unknown(x),
        }
    }
    pub fn to_byte(self, side: Side) -> u8 {
        match self {
            HoldAction::VoiceAssistant => 1,
            HoldAction::NoiseControl => 2,
            HoldAction::Volume => 3,
            HoldAction::Spotify => 4,
            HoldAction::Custom => match side {
                Side::Left => 5,
                Side::Right => 6,
            },
            HoldAction::Unknown(x) => x,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EqPreset {
    Off,
    BassBoost,
    Soft,
    Dynamic,
    Clear,
    TrebleBoost,
}

impl EqPreset {
    pub const ALL: [EqPreset; 6] = [
        EqPreset::Off,
        EqPreset::BassBoost,
        EqPreset::Soft,
        EqPreset::Dynamic,
        EqPreset::Clear,
        EqPreset::TrebleBoost,
    ];
    pub fn from_byte(b: u8) -> Option<Self> {
        Self::ALL.get(b as usize).copied()
    }
    pub fn to_byte(self) -> u8 {
        Self::ALL.iter().position(|&p| p == self).unwrap() as u8
    }
}

/// Which touch gestures the firmware reacts to ("advanced touch lock").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gestures {
    /// Whole touchpad locked.
    pub locked: bool,
    pub single_tap: bool,
    pub double_tap: bool,
    pub triple_tap: bool,
    pub hold: bool,
    pub double_tap_call: bool,
    pub hold_call: bool,
}

impl Default for Gestures {
    fn default() -> Self {
        Gestures {
            locked: false,
            single_tap: true,
            double_tap: true,
            triple_tap: true,
            hold: true,
            double_tap_call: true,
            hold_call: true,
        }
    }
}

impl Gestures {
    /// Layout of ExtendedStatus byte 10 on Buds2 .. Buds3 FE generation.
    pub fn from_status_byte(b: u8) -> Self {
        Gestures {
            hold: b & 0x01 != 0,
            triple_tap: b & 0x02 != 0,
            double_tap: b & 0x04 != 0,
            single_tap: b & 0x08 != 0,
            double_tap_call: b & 0x10 != 0,
            hold_call: b & 0x20 != 0,
            locked: b & 0x80 == 0,
        }
    }
}

/// Modes cycled by the firmware's touch-and-hold "noise control" action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoiseCycle {
    pub anc: bool,
    pub ambient: bool,
    pub off: bool,
}

impl NoiseCycle {
    pub fn is_valid(&self) -> bool {
        [self.anc, self.ambient, self.off].iter().filter(|x| **x).count() >= 2
    }
}

impl Default for NoiseCycle {
    fn default() -> Self {
        NoiseCycle { anc: true, ambient: true, off: false }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FitResult {
    Bad,
    Good,
    Failed,
    Unknown(u8),
}

impl FitResult {
    pub fn from_byte(b: u8) -> Self {
        match b {
            0 => FitResult::Bad,
            1 => FitResult::Good,
            2 => FitResult::Failed,
            x => FitResult::Unknown(x),
        }
    }
}
