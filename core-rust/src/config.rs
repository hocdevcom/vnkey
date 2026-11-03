//! Read Bamboo data formats without depending on its frontend or Go runtime.
use serde::Deserialize;
use std::{collections::BTreeMap, path::Path};
#[derive(Clone, Debug, Default)]
pub struct MacroTable {
    entries: BTreeMap<String, String>,
    pub ignore_case: bool,
}
impl MacroTable {
    pub fn parse(text: &str, ignore_case: bool) -> Self {
        let mut table = Self {
            ignore_case,
            ..Self::default()
        };
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with(['#', ';']) {
                continue;
            }
            let parts: Vec<_> = line.split(':').collect();
            // Bamboo accepts exactly one colon, including empty keys/values.
            if parts.len() != 2 {
                continue;
            }
            let key = if ignore_case {
                parts[0].trim().to_lowercase()
            } else {
                parts[0].trim().to_owned()
            };
            table.entries.insert(key, parts[1].trim().to_owned());
        }
        table
    }
    pub fn load(path: impl AsRef<Path>, ignore_case: bool) -> std::io::Result<Self> {
        Ok(Self::parse(&std::fs::read_to_string(path)?, ignore_case))
    }
    pub fn get(&self, key: &str) -> Option<&str> {
        let key = if self.ignore_case {
            key.to_lowercase()
        } else {
            key.to_owned()
        };
        self.entries.get(&key).map(String::as_str)
    }
}
#[derive(Clone, Debug, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct BambooConfig {
    pub input_method: String,
    pub flags: u32,
    pub default_input_mode: i32,
    pub input_mode_mapping: BTreeMap<String, i32>,
    pub input_method_definitions: BTreeMap<String, BTreeMap<String, String>>,
}
impl Default for BambooConfig {
    fn default() -> Self {
        Self {
            input_method: "Telex".into(),
            flags: 7,
            default_input_mode: 1,
            input_mode_mapping: BTreeMap::new(),
            input_method_definitions: BTreeMap::new(),
        }
    }
}
impl BambooConfig {
    pub fn parse(text: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(text)
    }
    pub fn load(path: impl AsRef<Path>) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self::parse(&std::fs::read_to_string(path)?)?)
    }
    pub fn is_excluded(&self, wm_class: &str) -> bool {
        self.input_mode_mapping
            .get(wm_class)
            .copied()
            .filter(|m| !wm_class.is_empty() && (1..=7).contains(m))
            .unwrap_or(self.default_input_mode)
            == 7
    }
    pub fn engine(&self) -> Option<crate::composition::Engine> {
        let method = if let Some(def) = self.input_method_definitions.get(&self.input_method) {
            crate::rules::InputMethod::from_definition(
                def.iter()
                    .filter_map(|(k, v)| Some((k.chars().next()?, v.clone()))),
            )
        } else {
            crate::rules::InputMethod::builtin(&self.input_method)?
        };
        Some(crate::composition::Engine::new(method, self.flags))
    }
}
