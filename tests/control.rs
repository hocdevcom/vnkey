use std::path::PathBuf;
#[cfg(all(target_os = "linux", feature = "config-app"))]
use std::process::Command;
use viet_core::settings::Settings;

fn directory(tag: &str) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("tests/artifacts")
        .join(format!("control-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&path).unwrap();
    path
}
#[test]
fn settings_replace_atomically_and_invalid_updates_leave_last_good_file() {
    let root = directory("save");
    let path = root.join("config.toml");
    let mut settings = Settings::default();
    settings.save(&path).unwrap();
    settings.input_method = "VNI".into();
    settings.enabled = false;
    settings.save(&path).unwrap();
    assert_eq!(Settings::load(&path).unwrap().input_method, "VNI");
    let before = std::fs::read(&path).unwrap();
    settings.input_method = "unknown".into();
    assert!(settings.save(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert!(!std::fs::read_dir(&root).unwrap().any(|e| e
        .unwrap()
        .path()
        .extension()
        .is_some_and(|ext| ext == "tmp")));
}
#[test]
fn editing_preserves_bamboo_reference_without_freezing_imported_rules() {
    let root = directory("import");
    let path = root.join("config.toml");
    let legacy = root.join("legacy.json");
    std::fs::write(&legacy, r#"{"InputModeMapping":{"editor":2}}"#).unwrap();
    let settings = Settings {
        bamboo_config: Some("legacy.json".into()),
        ..Settings::default()
    };
    settings.save(&path).unwrap();
    let mut editable = Settings::load_editable(&path).unwrap();
    editable.enabled = false;
    editable.save(&path).unwrap();
    assert!(Settings::load_editable(&path)
        .unwrap()
        .apps
        .overrides
        .is_empty());
    std::fs::write(&legacy, r#"{"InputModeMapping":{"editor":5}}"#).unwrap();
    assert_eq!(
        Settings::load(&path).unwrap().apps.resolve("editor"),
        viet_core::app_profile::OutputMode::BackspaceCommit
    );
}
#[test]
#[cfg(all(target_os = "linux", feature = "config-app"))]
fn controller_binary_updates_file_and_reports_offline_cores() {
    let root = directory("cli");
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_viet-config"))
            .args(args)
            .env("XDG_CONFIG_HOME", &root)
            .env("XDG_RUNTIME_DIR", root.join("runtime"))
            .output()
            .unwrap()
    };
    let result = run(&["--set-method", "VNI"]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(report["notified"], 0);
    let config = root.join("viet-ime/config.toml");
    assert_eq!(Settings::load(&config).unwrap().input_method, "VNI");
    assert!(run(&["--disable"]).status.success());
    assert!(!Settings::load(&config).unwrap().enabled);
    assert!(!run(&["--set-method", "invalid"]).status.success());
    assert_eq!(Settings::load(&config).unwrap().input_method, "VNI");
}
#[cfg(all(target_os = "linux", feature = "config-app"))]
#[test]
fn gui_uses_same_controller_without_framework_settings() {
    use std::os::unix::fs::PermissionsExt;
    let root = directory("gui");
    let zenity = root.join("zenity");
    std::fs::write(&zenity,"#!/bin/sh\nif [ \"$1\" = '--forms' ]; then printf 'VNI\\tEnglish\\thoà, uỷ\\n'; fi\nexit 0\n").unwrap();
    std::fs::set_permissions(&zenity, std::fs::Permissions::from_mode(0o700)).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_viet-config"))
        .arg("--gui")
        .env(
            "PATH",
            format!(
                "{}:{}",
                root.display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .env("XDG_CONFIG_HOME", &root)
        .env("XDG_RUNTIME_DIR", root.join("runtime"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let settings = Settings::load(&root.join("viet-ime/config.toml")).unwrap();
    assert_eq!(settings.input_method, "VNI");
    assert!(!settings.enabled);
    assert_eq!(settings.flags & 2, 0);
}
