//! Core-owned settings. Frontends never read configuration files.
use crate::{app_profile::AppProfiles, rules::InputMethod};
use serde::{Deserialize, Serialize};
use std::{
    io,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub input_method: String,
    pub enabled: bool,
    pub flags: u32,
    pub apps: AppProfiles,
    /// Optional legacy Bamboo JSON path; only application policy is imported.
    pub bamboo_config: Option<PathBuf>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            input_method: "Telex".into(),
            enabled: true,
            flags: 7,
            apps: AppProfiles::default(),
            bamboo_config: None,
        }
    }
}
impl Settings {
    pub fn validate(&self) -> io::Result<()> {
        if !matches!(self.input_method.as_str(), "Telex" | "VNI" | "Telex 2")
            || InputMethod::builtin(&self.input_method).is_none()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "input_method must be Telex, Telex 2 or VNI",
            ));
        }
        if self.flags & !7 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unsupported flags",
            ));
        }
        Ok(())
    }
    pub fn load(path: &Path) -> io::Result<Self> {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(e),
        };
        let mut settings: Self =
            toml::from_str(&text).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        if let Some(bamboo_path) = &settings.bamboo_config {
            let bamboo_path = if bamboo_path.is_absolute() {
                bamboo_path.clone()
            } else {
                path.parent().unwrap_or(Path::new(".")).join(bamboo_path)
            };
            let legacy = crate::config::BambooConfig::parse(&std::fs::read_to_string(bamboo_path)?)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            let mut imported = AppProfiles::from_bamboo(&legacy);
            // Explicit TOML rules have highest priority over imported exact keys.
            imported.overrides.extend(settings.apps.overrides.clone());
            settings.apps = imported;
        }
        settings.validate()?;
        Ok(settings)
    }
    pub fn config_path() -> io::Result<PathBuf> {
        if let Some(path) = std::env::var_os("XDG_CONFIG_HOME").filter(|s| !s.is_empty()) {
            return Ok(PathBuf::from(path).join("viet-ime/config.toml"));
        }
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is unset"))?;
        Ok(PathBuf::from(home).join(".config/viet-ime/config.toml"))
    }
}
