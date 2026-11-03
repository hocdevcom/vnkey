//! Arena nodes preserve Bamboo transformation identity without shared mutable pointers.
use crate::rules::{Effect, Rule};

#[derive(Clone, Debug)]
pub struct Transformation {
    pub rule: Rule,
    pub target: Option<usize>,
    pub is_upper_case: bool,
}
impl Transformation {
    pub fn append(key: char, upper: bool) -> Self {
        Self {
            rule: Rule::append(key, key),
            target: None,
            is_upper_case: upper,
        }
    }
    pub fn effect(rule: Rule, target: usize, upper: bool) -> Self {
        Self {
            rule,
            target: Some(target),
            is_upper_case: upper,
        }
    }
}
pub const VOWELS: &str = "aàáảãạăằắẳẵặâầấẩẫậeèéẻẽẹêềếểễệiìíỉĩịoòóỏõọôồốổỗộơờớởỡợuùúủũụưừứửữựyỳýỷỹỵ";
pub fn lower(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}
pub fn vowel(c: char) -> bool {
    VOWELS.contains(c)
}
pub fn tone(c: char) -> u8 {
    VOWELS
        .chars()
        .position(|v| v == c)
        .map_or(0, |p| (p % 6) as u8)
}
pub fn with_tone(c: char, t: u8) -> char {
    VOWELS
        .chars()
        .position(|v| v == c)
        .and_then(|p| VOWELS.chars().nth(p - p % 6 + t as usize))
        .unwrap_or(c)
}
pub fn family(c: char) -> &'static str {
    match c {
        'a' | 'â' | 'ă' => "aâă__",
        'e' | 'ê' => "eê___",
        'o' | 'ô' | 'ơ' => "oô_ơ_",
        'u' | 'ư' => "u__ư_",
        'd' | 'đ' => "d___đ",
        _ => "",
    }
}
pub fn mark(c: char) -> Option<u8> {
    family(c).chars().position(|v| v == c).map(|p| p as u8)
}
pub fn with_mark(c: char, m: u8) -> char {
    let t = tone(c);
    let plain = with_tone(c, 0);
    let result = family(plain)
        .chars()
        .nth(m as usize)
        .filter(|v| *v != '_')
        .unwrap_or(plain);
    with_tone(result, t)
}

/// Flatten a selected composition. Targets always refer to stable arena indices.
pub fn flatten(
    arena: &[Transformation],
    ids: &[usize],
    raw: bool,
    toneless: bool,
    markless: bool,
    lowercase: bool,
) -> String {
    let mut out = String::new();
    for &id in ids {
        let node = &arena[id];
        if node.rule.key == '\0' || (!raw && node.rule.effect != Effect::Append) {
            continue;
        }
        let mut c = if raw { node.rule.key } else { node.rule.on };
        if !raw {
            for &effect_id in ids {
                let effect = &arena[effect_id];
                if effect.target != Some(id) {
                    continue;
                }
                c = match effect.rule.effect {
                    Effect::Mark(5) => node.rule.key,
                    Effect::Mark(m) => with_mark(c, m),
                    Effect::Tone(t) => with_tone(c, t),
                    Effect::Append => c,
                };
            }
        }
        if toneless {
            c = with_tone(c, 0);
        }
        if markless {
            c = with_mark(c, 0);
        }
        if lowercase {
            c = lower(c);
        } else if node.is_upper_case {
            c = c.to_uppercase().next().unwrap_or(c);
        }
        out.push(c);
    }
    out
}

/// Bamboo's rightmost CVC split, including gi/qu qualification and giêng exception.
pub fn split(arena: &[Transformation], ids: &[usize]) -> (Vec<usize>, Vec<usize>, Vec<usize>) {
    let roots: Vec<_> = ids
        .iter()
        .copied()
        .filter(|i| arena[*i].target.is_none())
        .collect();
    let mut end = roots.len();
    while end > 0 && !vowel(arena[roots[end - 1]].rule.result) {
        end -= 1;
    }
    let mut start = end;
    while start > 0 && vowel(arena[roots[start - 1]].rule.result) {
        start -= 1;
    }
    let (mut fc, mut vo, mut lc) = (
        roots[..start].to_vec(),
        roots[start..end].to_vec(),
        roots[end..].to_vec(),
    );
    if vo.is_empty() && fc.is_empty() {
        fc = std::mem::take(&mut lc);
    }
    if fc.len() == 1 && !vo.is_empty() {
        let head = arena[fc[0]].rule.result;
        let first = arena[vo[0]].rule.result;
        if (head == 'g'
            && first == 'i'
            && vo.len() > 1
            && !(arena[vo[1]].rule.result == 'e' && !lc.is_empty()))
            || (head == 'q' && first == 'u')
        {
            fc.push(vo.remove(0));
        }
    }
    for part in [&mut fc, &mut vo, &mut lc] {
        let base = part.clone();
        for root in base {
            part.extend(
                ids.iter()
                    .copied()
                    .filter(|i| arena[*i].target == Some(root)),
            );
        }
    }
    (fc, vo, lc)
}
