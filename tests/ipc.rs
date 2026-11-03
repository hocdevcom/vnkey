use std::path::PathBuf;
use viet_core::{ipc::Runtime, settings::Settings};
#[test]
fn commands_validate_before_updating_settings() {
    let runtime = Runtime::isolated(
        Settings::default(),
        PathBuf::from("tests/artifacts/unused.toml"),
    )
    .unwrap();
    assert_eq!(runtime.snapshot().revision, 0);
    for json in [
        r#"{"command":"set_input_method","value":"bad"}"#,
        r#"{"command":"set_enabled","value":"false"}"#,
        r#"{"command":"status","unknown":1}"#,
        "not-json",
    ] {
        let reply: serde_json::Value = serde_json::from_str(&runtime.command(json)).unwrap();
        assert_eq!(reply["ok"], false, "{json}: {reply}");
        assert_eq!(runtime.snapshot().revision, 0);
    }
    runtime.command(r#"{"command":"toggle"}"#);
    assert!(!runtime.snapshot().settings.enabled);
    assert_eq!(runtime.snapshot().revision, 1);
    runtime.command(r#"{"command":"status"}"#);
    assert_eq!(runtime.snapshot().revision, 1);
}
#[test]
fn parse_toml_and_reject_unknown_keys() {
    let settings: Settings =
        toml::from_str("input_method='VNI'\nenabled=false\n[apps.rules]\nCode='BackspaceCommit'\n")
            .unwrap();
    settings.validate().unwrap();
    assert!(!settings.enabled);
    assert!(toml::from_str::<Settings>("enabld=false").is_err());
    let example: Settings =
        toml::from_str(include_str!("../core-rust/config.example.toml")).unwrap();
    example.validate().unwrap();
}

#[test]
fn core_loads_bamboo_application_rules_and_toml_overrides() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("tests/artifacts");
    std::fs::create_dir_all(&root).unwrap();
    let legacy_name = format!("legacy-{}.json", std::process::id());
    let legacy = root.join(&legacy_name);
    let config = root.join(format!("import-{}.toml", std::process::id()));
    std::fs::write(
        &legacy,
        r#"{"DefaultInputMode":2,"InputModeMapping":{"editor":5,"excluded":7}}"#,
    )
    .unwrap();
    std::fs::write(
        &config,
        format!("bamboo_config='{legacy_name}'\n[apps.rules]\neditor='Preedit'\n"),
    )
    .unwrap();
    let settings = Settings::load(&config).unwrap();
    use viet_core::app_profile::OutputMode;
    assert_eq!(settings.apps.resolve("editor"), OutputMode::Preedit);
    assert_eq!(settings.apps.resolve("excluded"), OutputMode::Passthrough);
    assert_eq!(settings.apps.resolve("other"), OutputMode::SurroundingText);
    let runtime = Runtime::isolated(settings, config.clone()).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&runtime.command(r#"{"command":"status"}"#))
            .unwrap()["ok"],
        true
    );
    std::fs::remove_file(config).unwrap();
    std::fs::remove_file(legacy).unwrap();
}

#[cfg(unix)]
#[test]
fn unix_socket_lifecycle_and_live_reload() {
    use std::{
        io::{BufRead, BufReader, Write},
        os::unix::{fs::PermissionsExt, net::UnixStream},
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("tests/artifacts");
    std::fs::create_dir_all(&root).unwrap();
    let directory = root.join(format!("ipc-{}", std::process::id()));
    let config = root.join(format!("config-{}.toml", std::process::id()));
    std::fs::write(&config, "input_method='VNI'\nenabled=false\n").unwrap();
    let runtime =
        Runtime::start_at(Settings::default(), config.clone(), directory.clone()).unwrap();
    let socket = runtime.socket_path().unwrap().to_owned();
    assert_eq!(
        std::fs::metadata(&socket).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let mut stream = UnixStream::connect(&socket).unwrap();
    writeln!(stream, "{{\"command\":\"reload\"}}").unwrap();
    let mut reply = String::new();
    BufReader::new(stream).read_line(&mut reply).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&reply).unwrap()["ok"],
        true
    );
    assert_eq!(runtime.snapshot().settings.input_method, "VNI");
    assert!(!runtime.snapshot().settings.enabled);
    std::fs::write(&config, "input_method='Telex'\nenabled=true\n").unwrap();
    let report = viet_core::control::notify_directory(&directory);
    assert_eq!(report.notified, 1);
    assert!(report.errors.is_empty());
    assert_eq!(runtime.snapshot().settings.input_method, "Telex");
    // A stalled client must not keep the background worker alive during unload.
    let _stalled = UnixStream::connect(&socket).unwrap();
    let before = std::time::Instant::now();
    drop(runtime);
    assert!(before.elapsed() < std::time::Duration::from_secs(2));
    assert!(!socket.exists());
    std::fs::remove_file(config).unwrap();
    std::fs::remove_dir(directory).unwrap();
}
