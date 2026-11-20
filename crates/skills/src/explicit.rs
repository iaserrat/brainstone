use std::collections::HashSet;

use crate::{Catalog, Skill};

impl Catalog {
    pub(crate) fn expand_mentions(&self, text: &str) -> String {
        let selected = self.mentioned(text);
        if selected.is_empty() {
            return text.to_owned();
        }
        let instructions = selected
            .iter()
            .map(|skill| crate::skill_content::explicit(skill))
            .collect::<Vec<_>>()
            .join("\n\n");
        format!("{instructions}\n\nUser request:\n{text}")
    }

    fn mentioned<'a>(&'a self, text: &'a str) -> Vec<&'a Skill> {
        let mut seen = HashSet::new();
        mentions(text)
            .filter(|name| seen.insert(*name))
            .filter_map(|name| self.get(name))
            .collect()
    }
}

fn mentions(text: &str) -> impl Iterator<Item = &str> {
    text.match_indices('$')
        .filter_map(|(index, _)| mention_at(text, index))
}

fn mention_at(text: &str, index: usize) -> Option<&str> {
    let rest = &text[index + 1..];
    let end = rest
        .find(|value: char| !(value.is_ascii_lowercase() || value.is_ascii_digit() || value == '-'))
        .unwrap_or(rest.len());
    (end > 0).then_some(&rest[..end])
}
