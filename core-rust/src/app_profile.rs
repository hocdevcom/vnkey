//! Application policy and transport-independent output plans.
//! Bamboo mode numbers are imported into three transports; core injects no events.
use crate::config::BambooConfig;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub enum OutputMode {
    #[default]
    Preedit,
    SurroundingText,
    BackspaceCommit,
    #[serde(rename = "Disabled")]
    Passthrough,
}
impl OutputMode {
    pub fn from_bamboo(value: i32) -> Option<Self> {
        Some(match value {
            1 => Self::Preedit,
            2 => Self::SurroundingText,
            3..=6 => Self::BackspaceCommit,
            7 => Self::Passthrough,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct AppProfiles {
    /// Exact user WM_CLASS/App ID keys, deliberately case-sensitive like Bamboo.
    #[serde(rename = "rules")]
    pub overrides: BTreeMap<String, OutputMode>,
    pub default_mode: OutputMode,
    pub use_builtin_profiles: bool,
}
impl Default for AppProfiles {
    fn default() -> Self {
        Self {
            overrides: BTreeMap::new(),
            default_mode: OutputMode::Preedit,
            use_builtin_profiles: true,
        }
    }
}
impl AppProfiles {
    /// Explicit DefaultInputMode remains authoritative for applications without a mapping.
    pub fn from_bamboo(config: &BambooConfig) -> Self {
        Self {
            overrides: config
                .input_mode_mapping
                .iter()
                .filter_map(|(app, mode)| Some((app.clone(), OutputMode::from_bamboo(*mode)?)))
                .collect(),
            default_mode: OutputMode::from_bamboo(config.default_input_mode).unwrap_or_default(),
            use_builtin_profiles: false,
        }
    }
    pub fn resolve(&self, app: &str) -> OutputMode {
        if !app.is_empty() {
            if let Some(mode) = self.overrides.get(app) {
                return *mode;
            }
            if self.use_builtin_profiles {
                // Match complete IDs or WM_CLASS components, never substrings/wildcards.
                for id in std::iter::once(app).chain(app.split(':')) {
                    if let Some(mode) = builtin_mode(&id.to_lowercase()) {
                        return mode;
                    }
                }
            }
        }
        self.default_mode
    }
}

fn builtin_mode(id: &str) -> Option<OutputMode> {
    // Bamboo's DefaultBrowserList supplies Firefox, Chrome and Chromium names.
    // Their transport choices and all other aliases below are viet-core policy.
    Some(match id {
        "navigator:firefox"
        | "google-chrome:google-chrome"
        | "chromium-browser:chromium-browser"
        | "firefox"
        | "org.mozilla.firefox"
        | "google-chrome"
        | "google-chrome-stable"
        | "chromium"
        | "chromium-browser"
        | "org.chromium.chromium" => OutputMode::SurroundingText,
        "code"
        | "code-insiders"
        | "com.visualstudio.code"
        | "codium"
        | "vscodium"
        | "telegramdesktop"
        | "telegram-desktop"
        | "org.telegram.desktop" => OutputMode::BackspaceCommit,
        "gnome-terminal"
        | "gnome-terminal-server"
        | "org.gnome.terminal"
        | "org.gnome.console"
        | "konsole"
        | "org.kde.konsole"
        | "alacritty"
        | "kitty"
        | "com.mitchellh.ghostty"
        | "org.wezfurlong.wezterm"
        | "xterm" => OutputMode::Preedit,
        _ => return None,
    })
}
