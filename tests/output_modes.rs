use std::{path::PathBuf, sync::Arc};
use viet_core::{
    app_profile::{AppProfiles, OutputMode},
    engine::*,
    ipc::Runtime,
    settings::Settings,
};
fn engine(mode: OutputMode) -> VietEngine {
    let mut settings = Settings::default();
    settings.apps.default_mode = mode;
    settings.apps.use_builtin_profiles = false;
    VietEngine::with_runtime(
        Runtime::isolated(settings, PathBuf::from("tests/artifacts/unused.toml")).unwrap(),
    )
}
fn scalar(c: char) -> u32 {
    if c as u32 <= 0xff {
        c as u32
    } else {
        0x0100_0000 | c as u32
    }
}
fn apply(screen: &mut String, preedit: &mut String, action: &Action) {
    *preedit = action.preedit.clone();
    if matches!(
        action.action_type,
        ActionType::DeleteSurrounding | ActionType::FakeBackspace
    ) {
        for _ in 0..action.backspace_count {
            assert!(screen.pop().is_some());
        }
    }
    screen.push_str(&action.commit);
}
#[test]
fn replay_transports_produces_the_same_text_with_scalar_deletion() {
    for mode in [
        OutputMode::Preedit,
        OutputMode::SurroundingText,
        OutputMode::BackspaceCommit,
    ] {
        let mut e = engine(mode);
        let mut screen = String::new();
        let mut preedit = String::new();
        for c in "tieengs Vieetj hoaf toasn ".chars() {
            let action = e.process_key(scalar(c), 0, CAP_SURROUNDING);
            apply(&mut screen, &mut preedit, &action);
        }
        assert_eq!(screen, "tiếng Việt hòa toán ");
        assert_eq!(preedit, "");
        assert!(e.core.composition().is_empty());
        e.process_key('a' as u32, 0, CAP_SURROUNDING);
        let action = e.process_key('s' as u32, 0, CAP_SURROUNDING);
        if mode != OutputMode::Preedit {
            assert_eq!(action.backspace_count, 1);
            assert_eq!(action.commit, "á");
        }
    }
}
#[test]
fn undo_backspace_and_empty_tail_never_delete_prior_commits() {
    let mut e = engine(OutputMode::BackspaceCommit);
    let mut screen = String::from("existing ");
    let mut preedit = String::new();
    for key in ['a' as u32, 's' as u32, 's' as u32, 0xff08, 0xff08] {
        let action = e.process_key(key, 0, 0);
        apply(&mut screen, &mut preedit, &action);
    }
    assert_eq!(screen, "existing ");
    assert_eq!(e.process_key(0xff08, 0, 0).action_type, ActionType::Ignore);
}
#[test]
fn exact_rules_precede_aliases_and_bamboo_import_keeps_exclusion() {
    let mut profiles = AppProfiles::default();
    assert_eq!(
        profiles.resolve("Navigator:Firefox"),
        OutputMode::SurroundingText
    );
    assert_eq!(
        profiles.resolve("gtk3-im:firefox"),
        OutputMode::SurroundingText
    );
    assert_eq!(
        profiles.resolve("com.visualstudio.code"),
        OutputMode::BackspaceCommit
    );
    assert_eq!(profiles.resolve("not-code-editor"), OutputMode::Preedit);
    profiles
        .overrides
        .insert("Code".into(), OutputMode::Preedit);
    assert_eq!(profiles.resolve("Code"), OutputMode::Preedit);
    let config = viet_core::config::BambooConfig::parse(
        r#"{"DefaultInputMode":2,"InputModeMapping":{"Code":7,"editor":5,"broken":99}}"#,
    )
    .unwrap();
    let imported = AppProfiles::from_bamboo(&config);
    assert_eq!(imported.resolve("Code"), OutputMode::Passthrough);
    assert_eq!(imported.resolve("editor"), OutputMode::BackspaceCommit);
    assert_eq!(imported.resolve("unknown"), OutputMode::SurroundingText);
}
#[test]
fn capability_fallback_focus_flush_navigation_shortcuts_and_releases() {
    let mut e = engine(OutputMode::SurroundingText);
    e.process_key('a' as u32, 0, 0);
    assert_eq!(
        e.process_key('s' as u32, 0, 0).action_type,
        ActionType::FakeBackspace
    );
    e.reset();
    e.process_key('a' as u32, 0, CAP_SURROUNDING);
    assert_eq!(
        e.process_key('s' as u32, 0, CAP_SURROUNDING).action_type,
        ActionType::DeleteSurrounding
    );
    let mut e = engine(OutputMode::Preedit);
    e.process_key('a' as u32, 0, 0);
    let released = e.process_key('a' as u32, 0, MOD_RELEASE);
    assert_eq!(released, Action::default());
    e.process_key(0xffe1, 0, MOD_SHIFT);
    assert_eq!(e.core.text(), "a");
    let shortcut = e.process_key('c' as u32, 0, MOD_CTRL);
    assert_eq!(shortcut.action_type, ActionType::Ignore);
    assert_eq!(shortcut.commit, "a");
    e.process_key('b' as u32, 0, 0);
    assert_eq!(e.process_key(0xff51, 0, 0).commit, "b"); // Left arrow flushes, passes through.
    e.process_key('c' as u32, 0, 0);
    assert_eq!(e.process_key(KEY_FLUSH, 0, 0).commit, "c");
    e.set_app_id("other");
    assert!(e.core.text().is_empty());
    e.process_key('d' as u32, 0, 0);
    e.set_app_id("other");
    assert!(e.core.text().is_empty());
    assert_eq!(e.process_key(0x0100_1ea1, 0, 0).preedit, "ạ");
    assert_eq!(e.process_key(0xff1b, 0, 0).preedit, "");
}
#[test]
fn shared_settings_keep_sessions_separate_and_commit_old_preedit_before_change() {
    let runtime = Runtime::isolated(
        Settings::default(),
        PathBuf::from("tests/artifacts/unused.toml"),
    )
    .unwrap();
    let mut a = VietEngine::with_runtime(Arc::clone(&runtime));
    let mut b = VietEngine::with_runtime(Arc::clone(&runtime));
    a.process_key('a' as u32, 0, 0);
    b.process_key('o' as u32, 0, 0);
    assert!(runtime
        .command(r#"{"command":"set_input_method","value":"VNI"}"#)
        .contains("\"ok\":true"));
    let action = a.process_key('e' as u32, 0, 0);
    assert_eq!(action.commit, "a");
    assert_eq!(action.preedit, "e");
    let action = a.process_key('6' as u32, 0, 0);
    assert_eq!(action.preedit, "ê");
    assert_eq!(b.process_key('u' as u32, 0, 0).commit, "o");
    runtime.command(r#"{"command":"set_enabled","value":false}"#);
    let action = a.process_key('x' as u32, 0, 0);
    assert_eq!(action.commit, "ê");
    assert_eq!(action.action_type, ActionType::Ignore);
    assert!(a.core.text().is_empty());
    assert_eq!(a.process_key('a' as u32, 0, SENSITIVE), Action::default());
}
