//! Rules and spelling matrices ported from BambooEngine/bamboo-core (MIT).
use crate::syllable::{family, mark, vowel, with_mark, with_tone};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    Append,
    Mark(u8),
    Tone(u8),
}
#[derive(Clone, Debug)]
pub struct Rule {
    pub key: char,
    pub on: char,
    pub result: char,
    pub effect: Effect,
    pub appended: Vec<char>,
}
impl Rule {
    pub fn append(key: char, c: char) -> Self {
        Self {
            key,
            on: c,
            result: c,
            effect: Effect::Append,
            appended: vec![],
        }
    }
    pub fn effect(key: char, effect: Effect) -> Self {
        Self {
            effect,
            ..Self::append(key, '\0')
        }
    }
}
#[derive(Clone, Debug)]
pub struct InputMethod {
    pub rules: Vec<Rule>,
    pub super_keys: Vec<char>,
    pub keys: Vec<char>,
}
impl InputMethod {
    pub fn from_definition(def: impl IntoIterator<Item = (char, String)>) -> Self {
        let mut im = Self {
            rules: vec![],
            super_keys: vec![],
            keys: vec![],
        };
        for (key, line) in def {
            im.keys.push(key);
            if line.to_lowercase().contains("uo") {
                im.super_keys.push(key);
            }
            im.rules.extend(parse_rule(key, &line));
        }
        im
    }
    pub fn telex() -> Self {
        Self::builtin("Telex").unwrap()
    }
    pub fn builtin(name: &str) -> Option<Self> {
        let defs: std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>> =
            serde_json::from_str(include_str!("input_methods.json")).ok()?;
        Some(Self::from_definition(
            defs.get(name)?
                .iter()
                .filter_map(|(k, v)| Some((k.chars().next()?, v.clone()))),
        ))
    }
}
pub fn parse_rule(key: char, line: &str) -> Vec<Rule> {
    let tone = match line {
        "XoaDauThanh" => Some(0),
        "DauHuyen" => Some(1),
        "DauSac" => Some(2),
        "DauHoi" => Some(3),
        "DauNga" => Some(4),
        "DauNang" => Some(5),
        _ => None,
    };
    if let Some(t) = tone {
        return vec![Rule::effect(key, Effect::Tone(t))];
    }
    let mut rules = vec![];
    let parts: Vec<_> = line.split('_').collect();
    if parts.len() < 2 {
        return rules;
    }
    if !parts[0].is_empty() {
        for (on, result) in parts[0]
            .to_lowercase()
            .chars()
            .zip(parts[1].to_lowercase().chars())
        {
            let Some(m) = mark(result) else { continue };
            for c in family(on).chars().filter(|c| *c != '_') {
                if c == result {
                    rules.push(Rule {
                        on: result,
                        result: on,
                        ..Rule::effect(key, Effect::Mark(0))
                    });
                } else if vowel(c) {
                    for t in 0..6 {
                        rules.push(Rule {
                            on: with_tone(c, t),
                            result: with_tone(result, t),
                            ..Rule::effect(key, Effect::Mark(m))
                        });
                    }
                } else {
                    rules.push(Rule {
                        on: c,
                        result,
                        ..Rule::effect(key, Effect::Mark(m))
                    });
                }
            }
        }
    }
    let suffix = if parts[0].is_empty() {
        parts.iter().skip(1).find(|s| !s.is_empty()).copied()
    } else {
        parts.iter().skip(2).find(|s| !s.is_empty()).copied()
    };
    if let Some(s) = suffix {
        let suffix = if parts[0].is_empty() {
            s.to_owned()
        } else {
            s.to_lowercase()
        };
        let mut chars = suffix.chars();
        if let Some(c) = chars.next() {
            let mut r = Rule::append(key, c);
            r.appended = chars.collect();
            rules.push(r);
        }
    }
    rules
}
const INITIAL: &[&str] = &[
    "b d đ g gh m n nh p ph r s t tr v z",
    "c h k kh qu th",
    "ch gi l ng ngh x",
    "đ l",
    "h",
];
const NUCLEUS: &[&str] = &["ê i ua uê uy y", "a iê oa uyê yê", "â ă e o oo ô ơ oe u ư uâ uô ươ", "oă", "uơ", "ai ao au âu ay ây eo êu ia iêu iu oai oao oay oeo oi ôi ơi ưa uây ui ưi uôi ươi ươu ưu uya uyu yêu", "ă", "i"];
const CODA: &[&str] = &["ch nh", "c ng", "m n p t", "k", "c"];
const CV: &[u8] = &[0b100111, 0b111111, 0b101111, 0b1000000, 0b10000000];
const VC: &[u8] = &[0b00101, 0b00111, 0b00110, 0b00110, 0, 0, 0b01000, 0b10000];
fn lookup(rows: &[&str], input: &str, full: bool, complete: bool) -> u8 {
    let mut bits = 0;
    for (i, row) in rows.iter().enumerate() {
        if row.split_whitespace().any(|s| {
            let a: Vec<_> = input.chars().collect();
            let b: Vec<_> = s.chars().collect();
            a.len() <= b.len()
                && (!full || a.len() == b.len())
                && a.iter()
                    .zip(b)
                    .all(|(a, b)| *a == b || (!complete && with_mark(b, 0) == *a))
        }) {
            bits |= 1 << i;
        }
    }
    bits
}
pub fn valid_cvc(fc: &str, vo: &str, lc: &str, complete: bool) -> bool {
    let f = if fc.is_empty() {
        0
    } else {
        lookup(INITIAL, fc, complete || !vo.is_empty(), true)
    };
    let v = if vo.is_empty() {
        0
    } else {
        lookup(NUCLEUS, vo, complete || !lc.is_empty(), complete)
    };
    let l = if lc.is_empty() {
        0
    } else {
        lookup(CODA, lc, complete, true)
    };
    if (!fc.is_empty() && f == 0) || (!vo.is_empty() && v == 0) || (!lc.is_empty() && l == 0) {
        return false;
    }
    if v == 0 {
        return f != 0;
    }
    if f != 0 && !(0..INITIAL.len()).any(|i| f & (1 << i) != 0 && CV[i] & v != 0) {
        return false;
    }
    l == 0 || (0..NUCLEUS.len()).any(|i| v & (1 << i) != 0 && VC[i] & l != 0)
}
