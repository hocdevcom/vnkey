//! Transformation engine adapted from Bamboo's Go implementation (MIT).
use crate::{
    rules::{Effect, InputMethod, Rule},
    syllable::*,
};
pub const FREE_MARK: u32 = 1;
/// Bamboo's standard style: hòa, ủy. Clear this bit for hoà, uỷ.
pub const STANDARD_TONE: u32 = 2;
pub const DEFAULT_FLAGS: u32 = 7;
#[derive(Clone, Debug)]
pub struct Engine {
    pub input_method: InputMethod,
    pub flags: u32,
    arena: Vec<Transformation>,
    composition: Vec<usize>,
}
impl Default for Engine {
    fn default() -> Self {
        Self::new(InputMethod::telex(), DEFAULT_FLAGS)
    }
}
impl Engine {
    pub fn new(input_method: InputMethod, flags: u32) -> Self {
        Self {
            input_method,
            flags,
            arena: vec![],
            composition: vec![],
        }
    }
    pub fn transformations(&self) -> &[Transformation] {
        &self.arena
    }
    pub fn composition(&self) -> &[usize] {
        &self.composition
    }
    fn push(&mut self, node: Transformation) -> usize {
        let id = self.arena.len();
        self.arena.push(node);
        id
    }
    fn text_of(&self, ids: &[usize], raw: bool, toneless: bool, markless: bool) -> String {
        flatten(&self.arena, ids, raw, toneless, markless, true)
    }
    pub fn text(&self) -> String {
        flatten(&self.arena, &self.composition, false, false, false, false)
    }
    pub fn raw(&self) -> String {
        flatten(&self.arena, &self.composition, true, false, false, false)
    }
    pub fn reset(&mut self) {
        self.arena.clear();
        self.composition.clear();
    }
    pub fn can_process(&self, c: char) -> bool {
        let c = lower(c);
        c.is_ascii_alphabetic()
            || self.input_method.keys.contains(&c)
            || tone(c) != 0
            || with_mark(c, 0) != c
    }
    fn last_word(&self, ids: &[usize], keys: &[char]) -> usize {
        for i in (0..ids.len()).rev() {
            if let Some(c) = self.text_of(&ids[i..], false, true, true).chars().next() {
                if !c.is_ascii_alphabetic() && !keys.contains(&c) {
                    return i + 1;
                }
            }
        }
        0
    }
    fn syllable_start(&self, ids: &[usize]) -> usize {
        let start = self.last_word(ids, &[]);
        let mut anchor = start;
        for i in start..ids.len() {
            if !self.valid_ids(&ids[anchor..=i], false) {
                anchor = i;
            }
        }
        anchor
    }
    fn last_append(&self, ids: &[usize]) -> Option<usize> {
        ids.iter()
            .rev()
            .copied()
            .find(|i| self.arena[*i].rule.effect == Effect::Append)
    }
    fn roots(&self, ids: &[usize]) -> Vec<usize> {
        ids.iter()
            .copied()
            .filter(|i| self.arena[*i].rule.effect == Effect::Append)
            .collect()
    }
    fn tone_valid(&self, ids: &[usize], t: u8) -> bool {
        if matches!(t, 0 | 2 | 5) {
            return true;
        }
        let (_, _, lc) = split(&self.arena, ids);
        !matches!(
            self.text_of(&lc, true, false, false).as_str(),
            "c" | "k" | "p" | "t" | "ch"
        )
    }
    fn valid_ids(&self, ids: &[usize], complete: bool) -> bool {
        if ids.len() <= 1 {
            return true;
        }
        if let Some(t) = ids
            .iter()
            .rev()
            .find_map(|i| match self.arena[*i].rule.effect {
                Effect::Tone(t) => Some(t),
                _ => None,
            })
        {
            if !self.tone_valid(ids, t) {
                return false;
            }
        }
        let (f, v, l) = split(&self.arena, ids);
        crate::rules::valid_cvc(
            &self.text_of(&f, false, true, false),
            &self.text_of(&v, false, true, false),
            &self.text_of(&l, false, true, false),
            complete,
        )
    }
    pub fn is_valid(&self, complete: bool) -> bool {
        let s = self.last_word(&self.composition, &self.input_method.keys);
        self.valid_ids(&self.composition[s..], complete)
    }
    fn tone_target(&self, ids: &[usize]) -> Option<usize> {
        let (_, vo, lc) = split(&self.arena, ids);
        let roots = self.roots(&vo);
        match roots.len() {
            1 => Some(roots[0]),
            2 if self.flags & STANDARD_TONE != 0 => {
                let special = vo
                    .iter()
                    .rfind(|i| matches!(self.arena[**i].rule.result, 'ơ' | 'ê'))
                    .copied();
                special
                    .map(|i| self.arena[i].target.unwrap_or(i))
                    .or_else(|| Some(roots[usize::from(!lc.is_empty())]))
            }
            2 => Some(
                roots[usize::from(
                    !lc.is_empty()
                        || matches!(
                            self.text_of(&roots, true, true, true).as_str(),
                            "oa" | "oe" | "uy" | "ue" | "uo"
                        ),
                )],
            ),
            3 => Some(
                roots[if self.text_of(&roots, true, true, true) == "uye" {
                    2
                } else {
                    1
                }],
            ),
            _ => None,
        }
    }
    fn effect_tone_target(&self, ids: &[usize], t: u8) -> Option<usize> {
        if self.flags & FREE_MARK != 0 {
            if self.tone_valid(ids, t) {
                self.tone_target(ids)
            } else {
                None
            }
        } else {
            self.last_append(ids)
                .filter(|i| vowel(self.arena[*i].rule.on))
        }
    }
    fn find_target(&mut self, ids: &[usize], rules: &[Rule]) -> Option<(usize, Rule)> {
        let before = self.text_of(ids, false, false, false);
        for rule in rules {
            if let Effect::Tone(t) = rule.effect {
                let target = self.effect_tone_target(ids, t)?;
                let probe = self.push(Transformation::effect(rule.clone(), target, false));
                let mut tmp = ids.to_vec();
                tmp.push(probe);
                if before == self.text_of(&tmp, false, false, false) {
                    continue;
                }
                if t == 0
                    && !ids.iter().any(|i| {
                        self.arena[*i].target == Some(target)
                            && matches!(self.arena[*i].rule.effect, Effect::Tone(_))
                    })
                    && tone(self.arena[target].rule.result) == 0
                {
                    return None;
                }
                return Some((target, rule.clone()));
            }
        }
        for &i in ids.iter().rev() {
            for rule in rules {
                if !matches!(rule.effect,Effect::Mark(m) if m>0)
                    || self.arena[i].rule.result != rule.on
                {
                    continue;
                }
                let mut root = i;
                while let Some(t) = self.arena[root].target {
                    root = t;
                }
                let probe = self.push(Transformation::effect(rule.clone(), root, false));
                let mut tmp = ids.to_vec();
                tmp.push(probe);
                if before != self.text_of(&tmp, false, false, false) && self.valid_ids(&tmp, false)
                {
                    return Some((root, rule.clone()));
                }
            }
        }
        None
    }
    fn undo(&mut self, ids: &[usize], rules: &[Rule]) -> Vec<usize> {
        let mut out = vec![];
        let before = self.text_of(ids, false, true, false);
        for rule in rules {
            match rule.effect {
                Effect::Tone(t) => {
                    if let Some(target) = self.effect_tone_target(ids, t) {
                        out.push(self.push(Transformation::effect(
                            Rule::effect('\0', Effect::Tone(0)),
                            target,
                            false,
                        )));
                    }
                }
                Effect::Mark(_) => {
                    for &i in ids.iter().rev() {
                        if self.arena[i].rule.result != rule.on {
                            continue;
                        }
                        let mut root = i;
                        while let Some(t) = self.arena[root].target {
                            root = t;
                        }
                        let probe = self.push(Transformation::effect(
                            Rule::effect('\0', Effect::Mark(0)),
                            root,
                            false,
                        ));
                        let mut tmp = ids.to_vec();
                        tmp.push(probe);
                        if before != self.text_of(&tmp, false, true, false) {
                            out.push(probe);
                        }
                    }
                }
                _ => {}
            }
        }
        out
    }
    fn generate(&mut self, ids: &[usize], rules: &[Rule], key: char, upper: bool) -> Vec<usize> {
        if let Some(&last) = ids.last() {
            let r = &self.arena[last].rule;
            if r.effect == Effect::Append && r.key == key && r.key != r.result {
                return vec![self.push(Transformation::effect(
                    Rule::effect('\0', Effect::Mark(5)),
                    last,
                    false,
                ))];
            }
        }
        if let Some((target, rule)) = self.find_target(ids, rules) {
            let is_mark = matches!(rule.effect, Effect::Mark(_));
            let first = self.push(Transformation::effect(rule, target, upper));
            let mut out = vec![first];
            let mut tmp = ids.to_vec();
            tmp.push(first);
            if !is_mark || self.valid_ids(&tmp, true) {
                return out;
            }
            if let Some((target, mut rule)) = self.find_target(&tmp, rules) {
                rule.key = '\0';
                out.push(self.push(Transformation::effect(rule, target, false)));
            }
            return out;
        }
        let text = self.text_of(ids, false, true, false);
        if text.contains("ưo") || text.contains("ươ") {
            let (_, vo, _) = split(&self.arena, ids);
            let roots = self.roots(&vo);
            if let Some(&first) = roots.first() {
                let probe = self.push(Transformation::effect(
                    Rule::effect('\0', Effect::Mark(0)),
                    first,
                    false,
                ));
                let mut tmp = ids.to_vec();
                tmp.push(probe);
                if let Some((target, rule)) = self.find_target(&tmp, rules) {
                    if target != first {
                        let second = self.push(Transformation::effect(rule, target, upper));
                        return vec![probe, second];
                    }
                }
            }
        }
        let mut out = self.undo(ids, rules);
        if !out.is_empty() {
            out.push(self.push(Transformation::append(key, upper)));
        }
        out
    }
    fn refresh(&mut self, ids: &mut Vec<usize>) {
        if self.flags & FREE_MARK == 0 || !self.valid_ids(ids, false) {
            return;
        }
        let Some(last) = ids.iter().rev().copied().find(|i| {
            self.arena[*i].target.is_some() && matches!(self.arena[*i].rule.effect, Effect::Tone(_))
        }) else {
            return;
        };
        let Some(target) = self.tone_target(ids) else {
            return;
        };
        if self.arena[last].target == Some(target) {
            return;
        }
        // Match Bamboo: retarget the previous tone node and append virtual overrides.
        self.arena[last].target = Some(target);
        ids.push(self.push(Transformation::effect(
            Rule::effect('\0', Effect::Tone(0)),
            target,
            false,
        )));
        let mut rule = self.arena[last].rule.clone();
        rule.key = '\0';
        ids.push(self.push(Transformation::effect(rule, target, false)));
    }
    pub fn process_key(&mut self, key: char) {
        self.process_key_mode(key, false);
    }
    pub fn process_key_mode(&mut self, key: char, raw: bool) {
        let upper = key.is_uppercase();
        let key = lower(key);
        if raw || !self.can_process(key) {
            let id = self.push(Transformation::append(key, upper));
            self.composition.push(id);
            return;
        }
        let start = self.syllable_start(&self.composition);
        let mut ids = self.composition[start..].to_vec();
        let rules: Vec<_> = self
            .input_method
            .rules
            .iter()
            .filter(|r| r.key == key)
            .cloned()
            .collect();
        let mut generated = self.generate(&ids, &rules, key, upper);
        if generated.is_empty() {
            let rule = rules
                .iter()
                .find(|r| r.effect == Effect::Append)
                .cloned()
                .unwrap_or_else(|| Rule::append(key, key));
            let appended = rule.appended.clone();
            let mut node = Transformation {
                is_upper_case: upper || rule.on.is_uppercase(),
                rule,
                target: None,
            };
            node.rule.on = lower(node.rule.on);
            node.rule.result = node.rule.on;
            generated.push(self.push(node));
            for c in appended {
                let mut node = Transformation::append('\0', upper || c.is_uppercase());
                node.rule.on = lower(c);
                node.rule.result = lower(c);
                generated.push(self.push(node));
            }
            let mut tmp = ids.clone();
            tmp.extend(&generated);
            let text = self.text_of(&tmp, false, true, false);
            let chars: Vec<_> = text.chars().collect();
            let shortcut = chars
                .windows(3)
                .any(|w| matches!((w[0], w[1]), ('u', 'ơ') | ('ư', 'o')) && w[2].is_alphabetic());
            if shortcut {
                if let Some(key) = self.input_method.super_keys.first().copied() {
                    let rules: Vec<_> = self
                        .input_method
                        .rules
                        .iter()
                        .filter(|r| r.key == key)
                        .cloned()
                        .collect();
                    if let Some((target, mut rule)) = self.find_target(&tmp, &rules) {
                        rule.key = '\0';
                        generated.push(self.push(Transformation::effect(rule, target, false)));
                    }
                }
            }
        }
        ids.extend(generated);
        self.refresh(&mut ids);
        self.composition.truncate(start);
        self.composition.extend(ids);
        self.compact();
    }
    /// Discard speculative probes and remap live arena links after each key.
    fn compact(&mut self) {
        let mut mapping = vec![usize::MAX; self.arena.len()];
        for (new, &old) in self.composition.iter().enumerate() {
            mapping[old] = new;
        }
        let mut arena = Vec::with_capacity(self.composition.len());
        for &old in &self.composition {
            let mut node = self.arena[old].clone();
            node.target = node
                .target
                .and_then(|i| mapping.get(i).copied())
                .filter(|i| *i != usize::MAX);
            arena.push(node);
        }
        self.arena = arena;
        self.composition = (0..self.arena.len()).collect();
    }
    pub fn process(&mut self, text: &str) {
        for c in text.chars() {
            self.process_key(c);
        }
    }
    pub fn backspace(&mut self, refresh: bool) {
        let Some(last) = self.last_append(&self.composition) else {
            return;
        };
        self.composition
            .retain(|i| *i != last && self.arena[*i].target != Some(last));
        if refresh {
            let start = self.last_word(&self.composition, &self.input_method.keys);
            let mut ids = self.composition[start..].to_vec();
            self.refresh(&mut ids);
            self.composition.truncate(start);
            self.composition.extend(ids);
        }
        self.compact();
    }
    pub fn restore_last_word(&mut self, vietnamese: bool) {
        let start = self.last_word(&self.composition, &self.input_method.keys);
        let keys: Vec<_> = self.composition[start..]
            .iter()
            .filter_map(|i| {
                let n = &self.arena[*i];
                (n.rule.key != '\0').then_some((n.rule.key, n.is_upper_case))
            })
            .collect();
        self.composition.truncate(start);
        for (key, upper) in keys {
            let key = if upper {
                key.to_uppercase().next().unwrap_or(key)
            } else {
                key
            };
            self.process_key_mode(key, !vietnamese);
        }
        self.compact();
    }
}
