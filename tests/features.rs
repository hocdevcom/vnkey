use viet_core::{
    composition::{Engine, DEFAULT_FLAGS},
    config::{BambooConfig, MacroTable},
    rules::InputMethod,
};

#[test]
fn requested_examples() {
    for (keys, want) in [
        ("ass", "as"),
        ("gisa", "giá"),
        ("gieengs", "giếng"),
        ("quawcj", "quặc"),
        ("nguwowfi", "người"),
        ("muoosi", "muối"),
        ("khuya", "khuya"),
        ("VIEETJ", "VIỆT"),
    ] {
        let mut e = Engine::default();
        e.process(keys);
        assert_eq!(e.text(), want, "{keys}");
    }
    for (flags, hoa, uy) in [(DEFAULT_FLAGS, "hòa", "ủy"), (5, "hoà", "uỷ")] {
        let mut e = Engine::new(InputMethod::telex(), flags);
        e.process("hoaf");
        assert_eq!(e.text(), hoa);
        e.reset();
        e.process("uyr");
        assert_eq!(e.text(), uy);
        e.reset();
        e.process("toas");
        e.process("n");
        assert_eq!(e.text(), "toán");
    }
}
#[test]
fn macro_and_exclusion_formats() {
    let table = MacroTable::parse(
        "#comment\n;comment\n VN : Việt Nam \nVN:mới\nbad:a:b\nempty:\n",
        true,
    );
    assert_eq!(table.get("vN"), Some("mới"));
    assert_eq!(table.get("bad"), None);
    assert_eq!(table.get("empty"), Some(""));
    let config = BambooConfig::parse(
        r#"{"InputMethod":"VNI","DefaultInputMode":1,"InputModeMapping":{"code:Code":7,"bad":99}}"#,
    )
    .unwrap();
    assert!(config.is_excluded("code:Code"));
    assert!(!config.is_excluded("Code"));
    assert!(!config.is_excluded("bad"));
    let mut e = config.engine().unwrap();
    e.process("Vie65t");
    assert_eq!(e.text(), "Việt");
}
#[test]
fn restore_and_delete() {
    let mut e = Engine::default();
    e.process("loanj");
    e.backspace(true);
    assert_eq!(e.text(), "lọa");
    e.reset();
    e.process("tooi");
    e.restore_last_word(false);
    assert_eq!(e.text(), "tooi");
    e.restore_last_word(true);
    assert_eq!(e.text(), "tôi");
}
#[test]
fn c_abi_borrowed_strings_and_nulls() {
    use viet_core::ffi::*;
    unsafe {
        let e = viet_engine_new();
        assert!(!e.is_null());
        viet_engine_set_app_id(e, c"unknown-app".as_ptr());
        let mut actual = String::new();
        for c in "tieengs Vieetj".chars() {
            let result = viet_engine_process_key(e, c as u32, 0, 0);
            assert!(!result.preedit_str.is_null());
            let commit = std::ffi::CStr::from_ptr(result.commit_str)
                .to_str()
                .unwrap();
            actual.push_str(commit);
        }
        let result = viet_engine_process_key(e, viet_core::engine::KEY_FLUSH, 0, 0);
        actual.push_str(
            std::ffi::CStr::from_ptr(result.commit_str)
                .to_str()
                .unwrap(),
        );
        assert_eq!(actual, "tiếng Việt");
        assert_eq!(
            viet_engine_process_key(std::ptr::null_mut(), 97, 0, 0).action_type,
            0
        );
        viet_engine_reset(e);
        viet_engine_free(e);
        viet_engine_free(std::ptr::null_mut());
    }
}
