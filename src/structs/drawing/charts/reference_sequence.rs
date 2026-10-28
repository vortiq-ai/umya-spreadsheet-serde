//! Registered reference lists retain every separator and quoted sheet token.
use super::Formula;
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct ReferenceSequence {
    references: Vec<Formula>,
    separators: Vec<String>,
    invalid:    bool,
}
impl Default for ReferenceSequence {
    fn default() -> Self {
        Self {
            references: Vec::new(),
            separators: vec![String::new()],
            invalid:    false,
        }
    }
}
impl ReferenceSequence {
    #[must_use]
    pub fn references(&self) -> &[Formula] {
        &self.references
    }

    pub fn references_mut(&mut self) -> &mut [Formula] {
        &mut self.references
    }

    #[must_use]
    pub fn separators(&self) -> &[String] {
        &self.separators
    }

    #[must_use]
    pub fn is_valid(&self) -> bool {
        !self.invalid && self.separators.len() == self.references.len() + 1
    }

    pub(crate) fn invalidate(&mut self) {
        self.invalid = true;
    }

    pub fn set_text(&mut self, text: &str) -> &mut Self {
        self.references.clear();
        self.separators.clear();
        self.invalid = false;
        let bytes = text.as_bytes();
        let mut at = 0;
        loop {
            let start = at;
            while at < bytes.len() && matches!(bytes[at], b' ' | b'\t' | b'\r' | b'\n' | b',') {
                at += 1;
            }
            self.separators.push(text[start..at].into());
            if at == bytes.len() {
                break;
            }
            let start = at;
            let mut quoted = false;
            while at < bytes.len() {
                if bytes[at] == b'\'' {
                    if quoted && bytes.get(at + 1) == Some(&b'\'') {
                        at += 2;
                        continue;
                    }
                    quoted = !quoted;
                    at += 1;
                    continue;
                }
                if !quoted && matches!(bytes[at], b' ' | b'\t' | b'\r' | b'\n' | b',') {
                    break;
                }
                at += 1;
            }
            self.invalid |= quoted;
            let mut formula = Formula::default();
            formula.set_address_str(&text[start..at]);
            self.references.push(formula);
        }
        self
    }

    #[must_use]
    pub fn text(&self) -> Option<String> {
        if !self.is_valid() {
            return None;
        }
        let mut text = self.separators[0].clone();
        for (i, reference) in self.references.iter().enumerate() {
            text.push_str(&reference.address_str());
            text.push_str(&self.separators[i + 1]);
        }
        Some(text)
    }
}
