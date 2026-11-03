//! Framework-independent input session: key -> application policy -> output action.
use crate::{
    app_profile::OutputMode,
    ipc::{Runtime, Snapshot},
    rules::InputMethod,
    Engine,
};
use std::{ffi::CString, io, sync::Arc};

pub const MOD_SHIFT: u32 = 1;
pub const MOD_CTRL: u32 = 2;
pub const MOD_ALT: u32 = 4;
pub const MOD_SUPER: u32 = 8;
pub const MOD_RELEASE: u32 = 1 << 16;
pub const MOD_FORWARDED: u32 = 1 << 17;
pub const CAP_SURROUNDING: u32 = 1 << 18;
pub const SENSITIVE: u32 = 1 << 19;
/// A lifecycle event encoded through the same five-function ABI, never a real key.
pub const KEY_FLUSH: u32 = u32::MAX;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u32)]
pub enum ActionType {
    #[default]
    Ignore = 0,
    Preedit = 1,
    Commit = 2,
    DeleteSurrounding = 3,
    FakeBackspace = 4,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Action {
    pub action_type: ActionType,
    pub backspace_count: u32,
    pub preedit: String,
    pub commit: String,
}

pub struct VietEngine {
    pub core: Engine,
    runtime: Arc<Runtime>,
    snapshot: Snapshot,
    app_id: String,
    mode: OutputMode,
    emitted: String,
    pub(crate) preedit_buf: CString,
    pub(crate) commit_buf: CString,
}
impl VietEngine {
    pub fn new() -> io::Result<Self> {
        Ok(Self::with_runtime(Runtime::shared()?))
    }
    pub fn with_runtime(runtime: Arc<Runtime>) -> Self {
        let snapshot = runtime.snapshot();
        let core = Engine::new(
            InputMethod::builtin(&snapshot.settings.input_method).expect("validated method"),
            snapshot.settings.flags,
        );
        Self {
            core,
            runtime,
            snapshot,
            app_id: String::new(),
            mode: OutputMode::Preedit,
            emitted: String::new(),
            preedit_buf: CString::default(),
            commit_buf: CString::default(),
        }
    }
    pub fn app_id(&self) -> &str {
        &self.app_id
    }
    pub fn output_mode(&self) -> OutputMode {
        self.mode
    }
    pub fn set_app_id(&mut self, app_id: &str) {
        self.reset();
        self.app_id = app_id.to_owned();
    }
    pub fn reset(&mut self) {
        self.core.reset();
        self.emitted.clear();
        self.preedit_buf = CString::default();
        self.commit_buf = CString::default();
    }
    fn clear_composition(&mut self) {
        self.core.reset();
        self.emitted.clear();
    }
    fn finish(&mut self) -> String {
        let commit = if self.mode == OutputMode::Preedit {
            self.core.text()
        } else {
            String::new()
        };
        self.clear_composition();
        commit
    }
    fn sync_settings(&mut self) -> String {
        let Some(snapshot) = self.runtime.snapshot_if_changed(self.snapshot.revision) else {
            return String::new();
        };
        let commit = self.finish();
        self.core = Engine::new(
            InputMethod::builtin(&snapshot.settings.input_method).expect("validated method"),
            snapshot.settings.flags,
        );
        self.snapshot = snapshot;
        commit
    }
    pub fn process_key(&mut self, keyval: u32, _keycode: u32, modifiers: u32) -> Action {
        if modifiers & (MOD_RELEASE | MOD_FORWARDED) != 0 {
            return Action::default();
        }
        if modifiers & SENSITIVE != 0 {
            self.clear_composition();
            return Action::default();
        }
        let mut pending = self.sync_settings();
        if keyval == KEY_FLUSH {
            pending.push_str(&self.finish());
            return Action {
                action_type: ActionType::Ignore,
                commit: pending,
                ..Action::default()
            };
        }
        let requested = self.snapshot.settings.apps.resolve(&self.app_id);
        let desired =
            if requested == OutputMode::SurroundingText && modifiers & CAP_SURROUNDING == 0 {
                OutputMode::BackspaceCommit
            } else {
                requested
            };
        if desired != self.mode {
            pending.push_str(&self.finish());
            self.mode = desired;
        }
        if !self.snapshot.settings.enabled || desired == OutputMode::Passthrough {
            pending.push_str(&self.finish());
            return Action {
                commit: pending,
                ..Action::default()
            };
        }
        let mut result = if modifiers & (MOD_CTRL | MOD_ALT | MOD_SUPER) != 0 {
            Action {
                commit: self.finish(),
                ..Action::default()
            }
        } else if (0xffe1..=0xffee).contains(&keyval) {
            // Modifier-only presses must not break a word (e.g. Shift for capitals).
            let preedit = if self.mode == OutputMode::Preedit {
                self.core.text()
            } else {
                String::new()
            };
            Action {
                preedit,
                ..Action::default()
            }
        } else {
            match keyval {
                0xff08 => {
                    if self.core.composition().is_empty() {
                        Action::default()
                    } else {
                        self.core.backspace(true);
                        self.output(false)
                    }
                }
                0xff1b => {
                    self.clear_composition();
                    Action {
                        action_type: if self.mode == OutputMode::Preedit {
                            ActionType::Preedit
                        } else {
                            ActionType::Ignore
                        },
                        ..Action::default()
                    }
                }
                value => {
                    if let Some(c) = printable(value) {
                        let boundary = !self.core.can_process(c);
                        self.core.process_key(c);
                        let action = self.output(boundary);
                        if boundary {
                            self.clear_composition();
                        }
                        action
                    } else {
                        Action {
                            commit: self.finish(),
                            ..Action::default()
                        }
                    }
                }
            }
        };
        if !pending.is_empty() {
            pending.push_str(&result.commit);
            result.commit = pending;
        }
        result
    }
    fn output(&mut self, boundary: bool) -> Action {
        let text = self.core.text();
        if self.mode == OutputMode::Preedit {
            return if boundary {
                Action {
                    action_type: ActionType::Commit,
                    commit: text,
                    ..Action::default()
                }
            } else {
                Action {
                    action_type: ActionType::Preedit,
                    preedit: text,
                    ..Action::default()
                }
            };
        }
        let common = self
            .emitted
            .chars()
            .zip(text.chars())
            .take_while(|(a, b)| a == b)
            .count();
        let deleted = (self.emitted.chars().count() - common) as u32;
        let commit: String = text.chars().skip(common).collect();
        self.emitted = text;
        Action {
            action_type: if deleted == 0 {
                ActionType::Commit
            } else if self.mode == OutputMode::SurroundingText {
                ActionType::DeleteSurrounding
            } else {
                ActionType::FakeBackspace
            },
            backspace_count: deleted,
            commit,
            preedit: String::new(),
        }
    }
}
fn printable(keyval: u32) -> Option<char> {
    let scalar = if keyval & 0xff00_0000 == 0x0100_0000 {
        keyval & 0x00ff_ffff
    } else if keyval <= 0xff {
        keyval
    } else {
        return None;
    };
    char::from_u32(scalar).filter(|c| !c.is_control() && *c != '\0')
}
