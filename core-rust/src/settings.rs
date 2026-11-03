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
        let mut settings = Self::load_editable(path)?;
        settings.apply_imports(path)?;
        Ok(settings)
    }
    /// Preserve explicit user rules when editing; do not flatten imported rules.
    pub fn load_editable(path: &Path) -> io::Result<Self> {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(e),
        };
        let settings: Self =
            toml::from_str(&text).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        settings.validate()?;
        Ok(settings)
    }
    fn apply_imports(&mut self, path: &Path) -> io::Result<()> {
        if let Some(bamboo_path) = &self.bamboo_config {
            let bamboo_path = if bamboo_path.is_absolute() {
                bamboo_path.clone()
            } else {
                path.parent().unwrap_or(Path::new(".")).join(bamboo_path)
            };
            let legacy = crate::config::BambooConfig::parse(&std::fs::read_to_string(bamboo_path)?)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            let mut imported = AppProfiles::from_bamboo(&legacy);
            // Explicit TOML rules have highest priority over imported exact keys.
            imported.overrides.extend(self.apps.overrides.clone());
            self.apps = imported;
        }
        Ok(())
    }
    pub fn save(&self, path: &Path) -> io::Result<()> {
        use std::{
            io::Write,
            sync::atomic::{AtomicU64, Ordering},
        };
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        self.validate()?;
        self.clone().apply_imports(path)?;
        let text = toml::to_string_pretty(self).map_err(io::Error::other)?;
        let parent = path
            .parent()
            .ok_or_else(|| io::Error::other("missing config directory"))?;
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(parent)?;
        let temporary = parent.join(format!(
            ".config-{}-{}.tmp",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        let result = (|| {
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options.open(&temporary)?;
            file.write_all(text.as_bytes())?;
            file.sync_all()?;
            drop(file);
            std::fs::rename(&temporary, path)
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&temporary);
        }
        result
    }
    pub fn runtime_directory(path: &Path) -> PathBuf {
        let base = std::env::var_os("XDG_RUNTIME_DIR")
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| path.parent().unwrap_or(Path::new(".")).join("run"));
        base.join("viet-ime")
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
