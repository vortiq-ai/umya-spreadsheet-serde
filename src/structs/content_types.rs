use std::collections::HashMap;

#[derive(Clone, Debug, Default)]
pub(crate) struct PackageContentTypes {
    overrides: Vec<(Box<str>, Box<str>)>,
    defaults:  HashMap<String, String>,
}

impl PackageContentTypes {
    pub(crate) fn insert_override(
        &mut self,
        part: String,
        content_type: String,
    ) -> Result<(), String> {
        validate_content_type(&content_type)?;
        if let Some((_, previous)) = self.overrides.iter().find(|(name, _)| {
            name.trim_start_matches('/')
                .eq_ignore_ascii_case(part.trim_start_matches('/'))
        }) {
            return if previous.as_ref() == content_type {
                Ok(())
            } else {
                Err(format!("conflicting content types for {part}"))
            };
        }
        self.overrides
            .push((part.into_boxed_str(), content_type.into_boxed_str()));
        Ok(())
    }

    pub(crate) fn insert_default(
        &mut self,
        mut extension: String,
        content_type: String,
    ) -> Result<(), String> {
        validate_content_type(&content_type)?;
        if let Some(previous) = self.defaults.get(&extension.to_ascii_lowercase()) {
            return if previous == &content_type {
                Ok(())
            } else {
                Err(format!(
                    "conflicting content types for extension {extension}"
                ))
            };
        }
        extension.make_ascii_lowercase();
        self.defaults.insert(extension, content_type);
        Ok(())
    }

    pub(crate) fn overrides(&self) -> &[(Box<str>, Box<str>)] {
        &self.overrides
    }

    pub(crate) fn set_overrides(&mut self, entries: Vec<(String, String)>) {
        self.overrides = entries
            .into_iter()
            .map(|(part, kind)| (part.into_boxed_str(), kind.into_boxed_str()))
            .collect();
    }

    pub(crate) fn get(&self, part: &str) -> Option<&str> {
        self.overrides
            .iter()
            .find(|(name, _)| {
                name.trim_start_matches('/')
                    .eq_ignore_ascii_case(part.trim_start_matches('/'))
            })
            .map(|(_, value)| value.as_ref())
            .or_else(|| {
                let extension = part.rsplit_once('.')?.1.to_ascii_lowercase();
                self.defaults.get(&extension).map(String::as_str)
            })
    }
}

pub(crate) fn validate_content_type(value: &str) -> Result<(), String> {
    fn token(bytes: &[u8], position: &mut usize) -> bool {
        let start = *position;
        while bytes.get(*position).is_some_and(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(
                    byte,
                    b'!' | b'#'
                        | b'$'
                        | b'%'
                        | b'&'
                        | b'\''
                        | b'*'
                        | b'+'
                        | b'-'
                        | b'.'
                        | b'^'
                        | b'_'
                        | b'`'
                        | b'|'
                        | b'~'
                )
        }) {
            *position += 1;
        }
        *position != start
    }

    fn valid(bytes: &[u8]) -> bool {
        let mut position = 0;
        if !token(bytes, &mut position) || bytes.get(position) != Some(&b'/') {
            return false;
        }
        position += 1;
        if !token(bytes, &mut position) {
            return false;
        }
        while position < bytes.len() {
            while bytes
                .get(position)
                .is_some_and(|byte| matches!(byte, b' ' | b'\t'))
            {
                position += 1;
            }
            if bytes.get(position) != Some(&b';') {
                return false;
            }
            position += 1;
            while bytes
                .get(position)
                .is_some_and(|byte| matches!(byte, b' ' | b'\t'))
            {
                position += 1;
            }
            if !token(bytes, &mut position) || bytes.get(position) != Some(&b'=') {
                return false;
            }
            position += 1;
            if bytes.get(position) != Some(&b'"') {
                if !token(bytes, &mut position) {
                    return false;
                }
                continue;
            }
            position += 1;
            loop {
                let Some(&byte) = bytes.get(position) else {
                    return false;
                };
                position += 1;
                match byte {
                    b'"' => break,
                    b'\\' => {
                        let Some(&escaped) = bytes.get(position) else {
                            return false;
                        };
                        if escaped != b'\t' && (escaped < b' ' || escaped == 0x7f) {
                            return false;
                        }
                        position += 1;
                    }
                    b'\t' | b' '..=b'!' | b'#'..=b'[' | b']'..=b'~' | 0x80..=0xff => (),
                    _ => return false,
                }
            }
        }
        true
    }

    if valid(value.as_bytes()) {
        Ok(())
    } else {
        Err(format!("invalid package content type: {value:?}"))
    }
}
