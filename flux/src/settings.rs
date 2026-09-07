use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub mode: Mode,
    pub animation: Animation,
    /// Simulation speed, independent of display frame rate (0.25–2.0).
    pub animation_speed: f32,
    pub seed: Option<String>,

    pub fluid_size: u32,
    pub fluid_frame_rate: f32,
    pub fluid_timestep: f32,
    pub viscosity: f32,
    pub velocity_dissipation: f32,
    pub pressure_mode: PressureMode,
    pub diffusion_iterations: u32,
    pub pressure_iterations: u32,

    pub color_mode: ColorMode,

    pub line_length: f32,
    pub line_width: f32,
    pub line_begin_offset: f32,
    pub line_variance: f32,
    pub grid_spacing: u32,
    pub view_scale: f32,

    pub noise_multiplier: f32,
    pub noise_channels: Vec<Noise>,

    /// User brightness multiplier (default: 1.0)
    /// Values < 1.0 dim, values > 1.0 brighten
    pub brightness_multiplier: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            mode: Mode::Normal,
            animation: Animation::Drift,
            animation_speed: 1.0,
            seed: None,
            fluid_size: 128,
            fluid_frame_rate: 60.0,
            fluid_timestep: 1.0 / 60.0,
            viscosity: 5.0,
            velocity_dissipation: 0.0,
            pressure_mode: PressureMode::ClearWith(0.0),
            diffusion_iterations: 3,
            pressure_iterations: 19,
            color_mode: ColorMode::Preset(ColorPreset::Original),
            line_length: 202.0,
            line_width: 9.0,
            line_begin_offset: 0.4,
            line_variance: 0.55,
            grid_spacing: 15,
            view_scale: 1.6,
            noise_multiplier: 0.45,
            noise_channels: vec![
                Noise {
                    scale: 2.8,
                    multiplier: 1.0,
                    offset_increment: 0.001,
                },
                Noise {
                    scale: 15.0,
                    multiplier: 0.7,
                    offset_increment: 0.001 * 6.0,
                },
                Noise {
                    scale: 30.0,
                    multiplier: 0.5,
                    offset_increment: 0.001 * 12.0,
                },
            ],
            brightness_multiplier: 1.0,
        }
    }
}

#[derive(Copy, Clone, Default, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[repr(u32)]
pub enum Animation {
    #[default]
    Drift,
    Silk,
    Ink,
    Topography,
}

impl Animation {
    pub const ALL: [Self; 4] = [Self::Drift, Self::Silk, Self::Ink, Self::Topography];
    pub const LABELS: [&'static str; 4] =
        ["Drift", "Flowing Silk", "Ink in Water", "Living Topography"];

    pub fn from_index(index: u32) -> Self {
        Self::ALL.get(index as usize).copied().unwrap_or_default()
    }
}

impl Settings {
    /// Surface modes emphasize broad structures without overwriting the user's channels.
    pub(crate) fn noise_profile(&self) -> Self {
        let mut profile = self.clone();
        for (index, channel) in profile.noise_channels.iter_mut().enumerate() {
            match self.animation {
                Animation::Silk => {
                    channel.scale *= 0.45;
                    if index > 0 {
                        channel.multiplier *= 0.045;
                    }
                }
                Animation::Topography => {
                    channel.scale *= 0.55;
                    if index > 0 {
                        channel.multiplier *= 0.12;
                    }
                }
                _ => {}
            }
        }
        profile
    }
}

#[derive(Clone, Default, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub enum Mode {
    #[default]
    Normal,
    DebugNoise,
    DebugFluid,
    DebugPressure,
    DebugDivergence,
}

#[derive(Copy, Clone, Debug, Deserialize, Serialize, PartialEq)]
pub enum PressureMode {
    Retain,
    ClearWith(f32),
}

impl Default for PressureMode {
    fn default() -> Self {
        Self::ClearWith(0.0)
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub enum ColorMode {
    Preset(ColorPreset),
    ImageFile(std::path::PathBuf),
    /// Six RGBA palette entries, retained across resizing and settings updates.
    Custom([f32; 24]),
}

impl ColorMode {
    pub fn to_color_wheel(&self) -> Option<[f32; 24]> {
        match self {
            Self::Preset(preset) => preset.to_color_wheel(),
            Self::Custom(colors) => Some(*colors),
            Self::ImageFile(_) => None,
        }
    }
}

impl Default for ColorMode {
    fn default() -> Self {
        Self::Preset(Default::default())
    }
}

impl From<ColorMode> for u32 {
    fn from(val: ColorMode) -> Self {
        match val {
            ColorMode::Preset(ColorPreset::Original) => 0,
            ColorMode::Preset(_) | ColorMode::Custom(_) => 1,
            ColorMode::ImageFile(_) => 2,
        }
    }
}

#[derive(Copy, Clone, Default, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub enum ColorPreset {
    #[default]
    Original,
    Plasma,
    Poolside,
    SpaceGrey,
}

impl ColorPreset {
    pub fn to_color_wheel(&self) -> Option<[f32; 24]> {
        match self {
            ColorPreset::Plasma => Some(COLOR_SCHEME_PLASMA),
            ColorPreset::Poolside => Some(COLOR_SCHEME_POOLSIDE),
            ColorPreset::SpaceGrey => Some(COLOR_SCHEME_SPACE_GREY),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Noise {
    pub scale: f32,
    pub multiplier: f32,
    pub offset_increment: f32,
}

#[rustfmt::skip]
pub static COLOR_SCHEME_PLASMA: [f32; 24] = [
    60.219  / 255.0, 37.2487 / 255.0, 66.4301 / 255.0, 1.0,
    170.962 / 255.0, 54.4873 / 255.0, 50.9661 / 255.0, 1.0,
    230.299 / 255.0, 39.2759 / 255.0, 5.54531 / 255.0, 1.0,
    242.924 / 255.0, 94.3563 / 255.0, 22.4186 / 255.0, 1.0,
    242.435 / 255.0, 156.752 / 255.0, 58.9794 / 255.0, 1.0,
    135.291 / 255.0, 152.793 / 255.0, 182.473 / 255.0, 1.0,
];

#[rustfmt::skip]
pub static COLOR_SCHEME_POOLSIDE: [f32; 24] = [
    76.0 / 255.0, 156.0 / 255.0, 228.0 / 255.0, 1.0,
    140.0 / 255.0, 204.0 / 255.0, 244.0 / 255.0, 1.0,
    108.0 / 255.0, 180.0 / 255.0, 236.0 / 255.0, 1.0,
    188.0 / 255.0, 228.0 / 255.0, 244.0 / 255.0, 1.0,
    124.0 / 255.0, 220.0 / 255.0, 236.0 / 255.0, 1.0,
    156.0 / 255.0, 208.0 / 255.0, 236.0 / 255.0, 1.0,
];

// Space Grey - grayscale scheme with saturation=0, luminance <75% (max ~191/255)
#[rustfmt::skip]
pub static COLOR_SCHEME_SPACE_GREY: [f32; 24] = [
    80.0 / 255.0, 80.0 / 255.0, 80.0 / 255.0, 1.0,      // Dark grey
    120.0 / 255.0, 120.0 / 255.0, 120.0 / 255.0, 1.0,   // Medium grey
    100.0 / 255.0, 100.0 / 255.0, 100.0 / 255.0, 1.0,   // Grey
    160.0 / 255.0, 160.0 / 255.0, 160.0 / 255.0, 1.0,   // Light grey
    140.0 / 255.0, 140.0 / 255.0, 140.0 / 255.0, 1.0,   // Medium-light grey
    180.0 / 255.0, 180.0 / 255.0, 180.0 / 255.0, 1.0,   // Lighter grey (70% luminance)
];

#[cfg(test)]
mod animation_tests {
    use super::*;
    #[test]
    fn old_settings_default_to_drift_and_normal_speed() {
        let settings: Settings = serde_json::from_str(r#"{"brightnessMultiplier":0.5}"#).unwrap();
        assert_eq!(settings.animation, Animation::Drift);
        assert_eq!(settings.animation_speed, 1.0);
        assert_eq!(settings.brightness_multiplier, 0.5);
        for animation in Animation::ALL {
            let settings = Settings {
                animation,
                animation_speed: 0.5,
                ..Default::default()
            };
            let restored: Settings =
                serde_json::from_str(&serde_json::to_string(&settings).unwrap()).unwrap();
            assert_eq!(settings, restored);
        }
    }
}
