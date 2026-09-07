use serde::{Deserialize, Serialize};

/// Persistent user preferences
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct UserPreferences {
    pub(crate) color_scheme: u32,
    pub(crate) animation: u32,
    pub(crate) animation_speed: u32,
    pub(crate) density: u32,
    pub(crate) noise_strength: u32,
    pub(crate) line_length: u32,
    pub(crate) line_width: u32,
    pub(crate) view_scale: u32,
    pub(crate) brightness: u32,
    pub(crate) fps: u32,
    #[serde(default)]
    pub(crate) run_on_login: bool,
    #[serde(default)]
    pub(crate) custom_color_wheel: Option<[f32; 24]>,
    #[serde(default)]
    pub(crate) custom_image_path: Option<String>,
}

impl Default for UserPreferences {
    fn default() -> Self {
        Self {
            color_scheme: 0,
            animation: 0,
            animation_speed: 1,
            density: 1,
            noise_strength: 1, // Medium
            line_length: 1,    // Medium
            line_width: 1,     // Medium
            view_scale: 1,     // Normal
            brightness: 1,     // Normal
            fps: 30,
            run_on_login: false,
            custom_color_wheel: None,
            custom_image_path: None,
        }
    }
}

pub(crate) fn get_preferences_path() -> std::path::PathBuf {
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").unwrap_or_default();
        std::path::PathBuf::from(format!("{}/.config/driftpaper/preferences.json", home))
    }
    #[cfg(target_os = "windows")]
    {
        let appdata = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
        std::path::PathBuf::from(format!("{}\\DriftPaper\\preferences.json", appdata))
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let home = std::env::var("HOME").unwrap_or_default();
        std::path::PathBuf::from(format!("{}/.config/driftpaper/preferences.json", home))
    }
}

static WRITE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

impl UserPreferences {
    fn validate(&mut self) {
        if self.animation >= flux::settings::Animation::ALL.len() as u32 {
            self.animation = 0;
        }
        if self.animation_speed > 2 {
            self.animation_speed = 1;
        }
        if !(1..=240).contains(&self.fps) {
            self.fps = 30;
        }
        if self.density > 2 {
            self.density = 1;
        }
        if self.noise_strength > 3 {
            self.noise_strength = 1;
        }
        if self.line_length > 3 {
            self.line_length = 1;
        }
        if self.line_width > 2 {
            self.line_width = 1;
        }
        if self.view_scale > 2 {
            self.view_scale = 1;
        }
        if self.brightness > 3 {
            self.brightness = 1;
        }
        if self.custom_color_wheel.is_some_and(|wheel| {
            wheel
                .iter()
                .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
        }) {
            self.custom_color_wheel = None;
        }
        if (!crate::palette::PALETTE_PRESETS
            .iter()
            .any(|entry| entry.0 == self.color_scheme)
            && self.color_scheme != 4)
            || (self.color_scheme == 4 && self.custom_color_wheel.is_none())
        {
            self.color_scheme = 0;
        }
    }
}

fn read_preferences(path: &std::path::Path) -> UserPreferences {
    let result = std::fs::read(path).and_then(|bytes| {
        serde_json::from_slice::<UserPreferences>(&bytes).map_err(std::io::Error::other)
    });
    match result {
        Ok(mut prefs) => {
            prefs.validate();
            prefs
        }
        Err(err) => {
            if err.kind() != std::io::ErrorKind::NotFound {
                log::warn!("Cannot load preferences at {}: {err}", path.display());
            }
            UserPreferences::default()
        }
    }
}

pub(crate) fn load_preferences() -> UserPreferences {
    read_preferences(&get_preferences_path())
}

fn write_preferences(path: &std::path::Path, prefs: &UserPreferences) -> std::io::Result<()> {
    use std::io::Write;
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::other("Preferences need a parent directory"))?;
    std::fs::create_dir_all(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    serde_json::to_writer_pretty(&mut temporary, prefs).map_err(std::io::Error::other)?;
    temporary.write_all(b"\n")?;
    temporary.as_file().sync_all()?;
    // Same-directory rename replaces the destination atomically on macOS and Windows.
    temporary.persist(path).map_err(|err| err.error)?;
    Ok(())
}

pub(crate) fn update_preferences(update: impl FnOnce(&mut UserPreferences)) {
    let _guard = WRITE_LOCK.lock().unwrap_or_else(|err| err.into_inner());
    let path = get_preferences_path();
    let mut prefs = read_preferences(&path);
    update(&mut prefs);
    prefs.validate();
    if let Err(err) = write_preferences(&path, &prefs) {
        log::error!("Cannot save preferences at {}: {err}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn old_preferences_gain_defaults_and_invalid_values_are_repaired() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("preferences.json");
        std::fs::write(&path, r#"{"density":2,"fps":0,"color_scheme":4}"#).unwrap();
        let prefs = read_preferences(&path);
        assert_eq!(prefs.density, 2);
        assert_eq!(prefs.fps, 30);
        assert_eq!(prefs.color_scheme, 0);
        assert_eq!(prefs.brightness, 1);
        assert_eq!(prefs.animation, 0);
        assert_eq!(prefs.animation_speed, 1);
    }
    #[test]
    fn atomic_replacement_preserves_complete_palette_and_fps() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("preferences.json");
        let mut prefs = UserPreferences::default();
        write_preferences(&path, &prefs).unwrap();
        prefs.fps = 60;
        prefs.animation = 2;
        prefs.animation_speed = 0;
        prefs.color_scheme = 4;
        prefs.custom_color_wheel = Some([0.5; 24]);
        write_preferences(&path, &prefs).unwrap();
        let loaded = read_preferences(&path);
        assert_eq!(loaded.fps, 60);
        assert_eq!(loaded.animation, 2);
        assert_eq!(loaded.animation_speed, 0);
        assert_eq!(loaded.custom_color_wheel, prefs.custom_color_wheel);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }
    #[test]
    fn every_palette_and_animation_survives_preferences_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("preferences.json");
        for (scheme, _, _, _) in crate::palette::PALETTE_PRESETS {
            for animation in 0..flux::settings::Animation::ALL.len() as u32 {
                let prefs = UserPreferences {
                    color_scheme: scheme,
                    animation,
                    custom_color_wheel: Some([0.5; 24]),
                    ..Default::default()
                };
                write_preferences(&path, &prefs).unwrap();
                let restored = read_preferences(&path);
                assert_eq!(
                    (restored.color_scheme, restored.animation),
                    (scheme, animation)
                );
                assert_eq!(restored.custom_color_wheel, prefs.custom_color_wheel);
            }
        }
        let prefs = UserPreferences {
            color_scheme: 4,
            custom_color_wheel: Some([0.3; 24]),
            ..Default::default()
        };
        write_preferences(&path, &prefs).unwrap();
        assert_eq!(read_preferences(&path).color_scheme, 4);
    }
    #[test]
    fn invalid_animation_preferences_preserve_other_settings() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("preferences.json");
        std::fs::write(
            &path,
            r#"{"animation":99,"animation_speed":99,"fps":60,"color_scheme":2}"#,
        )
        .unwrap();
        let prefs = read_preferences(&path);
        assert_eq!((prefs.animation, prefs.animation_speed), (0, 1));
        assert_eq!((prefs.fps, prefs.color_scheme), (60, 2));
    }
    #[test]
    fn failed_write_returns_an_error_without_replacing_destination() {
        let dir = tempfile::tempdir().unwrap();
        assert!(write_preferences(dir.path(), &UserPreferences::default()).is_err());
        assert!(dir.path().is_dir());
    }
}
