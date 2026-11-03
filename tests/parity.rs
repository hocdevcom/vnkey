use viet_core::{rules::InputMethod, Engine};
#[test]
fn go_reference() {
    let cases: serde_json::Value = serde_json::from_str(include_str!("go_reference.json")).unwrap();
    let mut mismatches = vec![];
    let mut count = 0;
    for case in cases.as_array().unwrap() {
        let method = case["Method"].as_str().unwrap();
        let flags = case["Flags"].as_u64().unwrap() as u32;
        let keys = case["Keys"].as_str().unwrap();
        let mut e = Engine::new(InputMethod::builtin(method).unwrap(), flags);
        for (index, (c, state)) in keys
            .chars()
            .zip(case["states"].as_array().unwrap())
            .enumerate()
        {
            if c == '\u{8}' {
                e.backspace(true);
            } else {
                e.process_key(c);
            }
            count += 1;
            if e.text() != state["Text"].as_str().unwrap()
                || e.raw() != state["Raw"].as_str().unwrap()
                || e.is_valid(false) != state["Valid"].as_bool().unwrap()
            {
                if mismatches.len() < 30 {
                    mismatches.push(format!(
                        "{method} flags={flags} {keys:?}@{index}: {:?}/{:?}/{} != {state}",
                        e.text(),
                        e.raw(),
                        e.is_valid(false)
                    ));
                }
                break;
            }
        }
    }
    assert!(
        mismatches.is_empty(),
        "{}\nchecked {count}",
        mismatches.join("\n")
    );
}
