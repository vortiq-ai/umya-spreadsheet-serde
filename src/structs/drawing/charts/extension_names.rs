//! Namespace context supplied by the chart reader at the extension boundary.
//! Prefix spellings never imply a namespace; inherited bindings are mandatory.
use quick_xml::events::BytesStart;

const XML_NAMESPACE: &str = "http://www.w3.org/XML/1998/namespace";
const XMLNS_NAMESPACE: &str = "http://www.w3.org/2000/xmlns/";

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QName {
    pub namespace_uri:    String,
    pub local_name:       String,
    pub preferred_prefix: Option<String>,
}
impl QName {
    #[must_use]
    pub fn is(&self, uri: &str, local: &str) -> bool {
        self.namespace_uri == uri && self.local_name == local
    }

    #[must_use]
    pub fn lexical(&self) -> String {
        match &self.preferred_prefix {
            Some(prefix) => format!("{prefix}:{}", self.local_name),
            None => self.local_name.clone(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct NamespaceScope {
    bindings: Vec<(Option<String>, String)>,
}
impl Default for NamespaceScope {
    fn default() -> Self {
        Self {
            bindings: vec![(
                Some("xml".into()),
                "http://www.w3.org/XML/1998/namespace".into(),
            )],
        }
    }
}
impl NamespaceScope {
    #[must_use]
    pub fn bindings(&self) -> &[(Option<String>, String)] {
        &self.bindings
    }

    #[must_use]
    pub fn effective_bindings(&self) -> Vec<(Option<String>, String)> {
        let mut result: Vec<(Option<String>, String)> = Vec::new();
        for (prefix, uri) in &self.bindings {
            if let Some((_, old)) = result.iter_mut().find(|(p, _)| p == prefix) {
                old.clone_from(uri);
            } else {
                result.push((prefix.clone(), uri.clone()));
            }
        }
        result
    }

    pub fn with_bindings(bindings: impl IntoIterator<Item = (Option<String>, String)>) -> Self {
        let mut scope = Self::default();
        scope.bindings.extend(bindings);
        scope
    }

    pub fn child(&self, element: &BytesStart) -> Result<Self, String> {
        let mut result = self.clone();
        result.bindings.extend(Self::declarations(element)?);
        Ok(result)
    }

    pub fn declarations(element: &BytesStart) -> Result<Vec<(Option<String>, String)>, String> {
        let mut result = Vec::new();
        for attr in element.attributes() {
            let attr = attr.map_err(|e| e.to_string())?;
            let raw = attr.key.into_inner();
            let prefix = if raw == b"xmlns" {
                None
            } else if let Some(prefix) = raw.strip_prefix(b"xmlns:") {
                Some(
                    std::str::from_utf8(prefix)
                        .map_err(|e| e.to_string())?
                        .to_owned(),
                )
            } else {
                continue;
            };
            let uri = attr
                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .map_err(|e| e.to_string())?
                .into_owned();
            if prefix.as_deref() == Some("xmlns")
                || uri == XMLNS_NAMESPACE
                || (prefix.as_deref() == Some("xml")) != (uri == XML_NAMESPACE)
                || (prefix.is_some() && uri.is_empty())
            {
                return Err("invalid reserved namespace binding".into());
            }
            result.push((prefix, uri));
        }
        Ok(result)
    }

    pub fn resolve(&self, raw: &[u8], attribute: bool) -> Result<QName, String> {
        let raw = std::str::from_utf8(raw).map_err(|e| e.to_string())?;
        let (prefix, local) = match raw.split_once(':') {
            Some((p, l)) => (Some(p), l),
            None => (None, raw),
        };
        if !valid_ncname(local) || prefix.is_some_and(|p| !valid_ncname(p)) {
            return Err("invalid QName".into());
        }
        let uri = if attribute && prefix.is_none() {
            ""
        } else {
            self.bindings
                .iter()
                .rev()
                .find(|(p, _)| p.as_deref() == prefix)
                .map(|(_, u)| u.as_str())
                .or_else(|| prefix.is_none().then_some(""))
                .ok_or_else(|| format!("undeclared prefix in {raw}"))?
        };
        if prefix.is_some() && uri.is_empty() {
            return Err("prefixed QName has empty namespace".into());
        }
        Ok(QName {
            namespace_uri:    uri.to_owned(),
            local_name:       local.to_owned(),
            preferred_prefix: prefix.map(str::to_owned),
        })
    }
}

fn valid_ncname(value: &str) -> bool {
    fn start(c: char) -> bool {
        matches!(c,
            'A'..='Z' | 'a'..='z' | '_' | '\u{c0}'..='\u{d6}' |
            '\u{d8}'..='\u{f6}' | '\u{f8}'..='\u{2ff}' |
            '\u{370}'..='\u{37d}' | '\u{37f}'..='\u{1fff}' |
            '\u{200c}'..='\u{200d}' | '\u{2070}'..='\u{218f}' |
            '\u{2c00}'..='\u{2fef}' | '\u{3001}'..='\u{d7ff}' |
            '\u{f900}'..='\u{fdcf}' | '\u{fdf0}'..='\u{fffd}' |
            '\u{10000}'..='\u{effff}'
        )
    }
    let mut chars = value.chars();
    chars.next().is_some_and(start) && chars.all(|c| {
        start(c)
            || matches!(c,
                '-' | '.' | '0'..='9' | '\u{b7}' | '\u{300}'..='\u{36f}' | '\u{203f}'..='\u{2040}'
            )
    })
}
