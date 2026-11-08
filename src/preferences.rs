use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub schema_version: u32,
    pub dark_theme: bool,
    pub sidebar_visible: bool,
    pub view_mode: u8,
    pub printer_name: String,
    pub print_fit: bool,
    /// Local crash-recovery checkpoints of unsaved drafts (PDF and mark images).
    pub recovery_enabled: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            schema_version: 1,
            dark_theme: false,
            sidebar_visible: true,
            view_mode: 0,
            printer_name: String::new(),
            print_fit: true,
            recovery_enabled: true,
        }
    }
}

pub fn preferences_path() -> Option<PathBuf> {
    if let Some(directory) = std::env::var_os("DPDF_CONFIG_DIR") {
        return Some(PathBuf::from(directory).join("preferences.json"));
    }
    #[cfg(target_os = "windows")]
    let directory = std::env::var_os("APPDATA").map(PathBuf::from);
    #[cfg(not(target_os = "windows"))]
    let directory = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")));
    directory.map(|p| p.join("dPDF").join("preferences.json"))
}

pub fn load(path: &Path) -> Result<Preferences> {
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Preferences::default());
        }
        Err(error) => return Err(error).context("Read preferences metadata"),
    };
    if metadata.len() > 65_536 {
        bail!("Preferences file is too large");
    }
    let mut preferences: Preferences =
        serde_json::from_slice(&std::fs::read(path)?).context("Decode preferences")?;
    if preferences.schema_version != 1 {
        bail!(
            "Unsupported preferences version {}",
            preferences.schema_version
        );
    }
    if preferences.view_mode > 2 {
        preferences.view_mode = 0;
    }
    Ok(preferences)
}

pub fn save(path: &Path, preferences: &Preferences) -> Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    crate::storage::atomic_write(&serde_json::to_vec_pretty(preferences)?, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preferences_round_trip_without_persisting_document_or_search_data() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("nested/preferences.json");
        let preferences = Preferences {
            dark_theme: true,
            sidebar_visible: false,
            view_mode: 2,
            printer_name: "Office printer \"north\"".into(),
            print_fit: false,
            ..Default::default()
        };
        save(&path, &preferences).unwrap();
        assert_eq!(load(&path).unwrap(), preferences);
        let json = std::fs::read_to_string(&path).unwrap();
        assert!(!json.contains("document"));
        assert!(!json.contains("query"));
        save(&path, &Preferences::default()).unwrap();
        assert_eq!(load(&path).unwrap(), Preferences::default());
        assert_eq!(
            std::fs::read_dir(path.parent().unwrap()).unwrap().count(),
            1
        );
    }

    #[test]
    fn absent_and_older_fields_default_but_corrupt_or_future_formats_fail() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("preferences.json");
        assert_eq!(load(&path).unwrap(), Preferences::default());
        std::fs::write(&path, r#"{"dark_theme":true,"view_mode":99}"#).unwrap();
        let loaded = load(&path).unwrap();
        assert!(loaded.dark_theme);
        assert_eq!(loaded.view_mode, 0);
        assert!(loaded.print_fit);
        assert!(loaded.recovery_enabled);
        std::fs::write(&path, r#"{"recovery_enabled":false}"#).unwrap();
        assert!(!load(&path).unwrap().recovery_enabled);
        for invalid in [
            "invalid json",
            r#"{"schema_version":2}"#,
            r#"{"print_fit":"no"}"#,
        ] {
            std::fs::write(&path, invalid).unwrap();
            assert!(load(&path).is_err());
        }
    }
}
