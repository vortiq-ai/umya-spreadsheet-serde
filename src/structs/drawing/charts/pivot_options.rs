//! `PivotChart` options Excel keeps in the chart-space extension list: the
//! field buttons (`c14:pivotOptions`) and the expand/collapse buttons
//! (`c16:pivotOptions16`).
//!
//! Excel writes a flag only when it is on and leaves it out when it is off, so
//! the setters follow that: `true` writes the element with `val="1"`, `false`
//! removes it. An extension whose content is not exactly the known flags with
//! an explicit `val` of `0` or `1` stays in the opaque extension list.
use super::{
    Extension,
    ExtensionList,
    ExtensionNode,
    OpaqueElement,
    OpaqueStart,
    OpaqueToken,
    QName,
    extension_registry::{
        NS_C,
        NS_C14,
        NS_C16,
    },
};

pub(crate) const URI_FIELD_BUTTONS: &str = "{781A3756-C4B2-4CAC-9D66-4F8BD8637D16}";
pub(crate) const URI_EXPAND_COLLAPSE: &str = "{E28EC0CA-F0BB-4C9C-879D-F8772B89E7AC}";
const FIELD_BUTTON_ELEMENTS: [&str; 5] = [
    "dropZoneFilter",
    "dropZoneCategories",
    "dropZoneData",
    "dropZoneSeries",
    "dropZonesVisible",
];
const EXPAND_COLLAPSE_ELEMENT: &str = "showExpandCollapseFieldButtons";
const KINDS: [(&str, &str, &str, &[&str]); 2] = [
    (
        URI_FIELD_BUTTONS,
        NS_C14,
        "pivotOptions",
        &FIELD_BUTTON_ELEMENTS,
    ),
    (
        URI_EXPAND_COLLAPSE,
        NS_C16,
        "pivotOptions16",
        &[EXPAND_COLLAPSE_ELEMENT],
    ),
];

/// The pivot field-button settings of a `PivotChart` (Excel's "Show Field
/// Buttons" menu and its per-zone entries).
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct PivotChartOptions {
    report_filter:            Option<bool>,
    axis:                     Option<bool>,
    values:                   Option<bool>,
    legend:                   Option<bool>,
    all:                      Option<bool>,
    expand_collapse:          Option<bool>,
    #[cfg_attr(feature = "serde", serde(default))]
    field_buttons_position:   usize,
    #[cfg_attr(feature = "serde", serde(default))]
    expand_collapse_position: usize,
}

macro_rules! flag {
    ($field:ident, $get:ident, $set:ident, $doc:literal) => {
        #[doc = $doc]
        #[must_use]
        pub fn $get(&self) -> bool {
            self.$field == Some(true)
        }

        pub fn $set(&mut self, value: bool) -> &mut Self {
            self.$field = value.then_some(true);
            self
        }
    };
}

impl PivotChartOptions {
    flag!(
        report_filter,
        show_report_filter_buttons,
        set_show_report_filter_buttons,
        "Report filter field buttons (`dropZoneFilter`)."
    );

    flag!(
        axis,
        show_axis_buttons,
        set_show_axis_buttons,
        "Axis (category) field buttons (`dropZoneCategories`)."
    );

    flag!(
        values,
        show_value_buttons,
        set_show_value_buttons,
        "Value field buttons (`dropZoneData`)."
    );

    flag!(
        legend,
        show_legend_buttons,
        set_show_legend_buttons,
        "Legend (series) field buttons (`dropZoneSeries`)."
    );

    flag!(
        all,
        show_field_buttons,
        set_show_field_buttons,
        "The overall \"Show Field Buttons\" switch (`dropZonesVisible`)."
    );

    flag!(
        expand_collapse,
        show_expand_collapse_buttons,
        set_show_expand_collapse_buttons,
        "Expand/collapse field buttons (`showExpandCollapseFieldButtons`)."
    );

    fn field_flags(&self) -> [Option<bool>; 5] {
        [
            self.report_filter,
            self.axis,
            self.values,
            self.legend,
            self.all,
        ]
    }

    /// Whether no flag is set, so neither extension is written.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.field_flags().iter().all(Option::is_none) && self.expand_collapse.is_none()
    }

    /// Takes the pivot option extensions out of a freshly read list. Each
    /// kind is taken only when it occurs once and holds nothing but flags.
    pub(crate) fn extract(list: &mut ExtensionList) -> Option<Self> {
        let mut taken: Vec<(usize, usize, Vec<Option<bool>>)> = Vec::new();
        for (kind, (uri, namespace, root, names)) in KINDS.iter().enumerate() {
            let mut matching = list
                .entries
                .iter()
                .enumerate()
                .filter(|(_, entry)| has_uri(entry, uri));
            if let (Some((index, entry)), None) = (matching.next(), matching.next()) {
                if let Some(flags) = read_flags(entry, namespace, root, names) {
                    taken.push((kind, index, flags));
                }
            }
        }
        if taken.is_empty() {
            return None;
        }
        let mut options = Self::default();
        for (kind, index, flags) in &taken {
            // The position counts the entries that stay in the list.
            let position = index - taken.iter().filter(|other| other.1 < *index).count();
            if *kind == 0 {
                options.report_filter = flags[0];
                options.axis = flags[1];
                options.values = flags[2];
                options.legend = flags[3];
                options.all = flags[4];
                options.field_buttons_position = position;
            } else {
                options.expand_collapse = flags[0];
                options.expand_collapse_position = position;
            }
        }
        let mut indices: Vec<usize> = taken.iter().map(|taken| taken.1).collect();
        indices.sort_unstable_by(|a, b| b.cmp(a));
        for index in indices {
            list.entries.remove(index);
        }
        Some(options)
    }

    /// Puts the typed extensions back into `list`. An opaque occurrence of
    /// the same kind remains authoritative: its content cannot be represented
    /// by the typed flags without loss.
    pub(crate) fn insert_into(&self, list: &mut ExtensionList) {
        let field_buttons = self.field_flags().iter().any(Option::is_some)
            && !list
                .entries
                .iter()
                .any(|entry| has_uri(entry, URI_FIELD_BUTTONS));
        let mut typed = Vec::new();
        if field_buttons {
            typed.push((
                self.field_buttons_position,
                0,
                build(
                    URI_FIELD_BUTTONS,
                    ("c14", NS_C14),
                    "pivotOptions",
                    &FIELD_BUTTON_ELEMENTS,
                    &self.field_flags(),
                ),
            ));
        }
        if self.expand_collapse.is_some()
            && !list
                .entries
                .iter()
                .any(|entry| has_uri(entry, URI_EXPAND_COLLAPSE))
        {
            typed.push((
                self.expand_collapse_position,
                1,
                build(
                    URI_EXPAND_COLLAPSE,
                    ("c16", NS_C16),
                    "pivotOptions16",
                    &[EXPAND_COLLAPSE_ELEMENT],
                    &[self.expand_collapse],
                ),
            ));
        }
        typed.sort_by_key(|(position, kind, _)| (*position, *kind));
        for (placed, (position, _, entry)) in typed.into_iter().enumerate() {
            let at = (position + placed).min(list.entries.len());
            list.entries.insert(at, entry);
        }
    }

    pub(crate) fn remove_from(list: &mut ExtensionList) {
        list.entries.retain(|entry| {
            !has_uri(entry, URI_FIELD_BUTTONS) && !has_uri(entry, URI_EXPAND_COLLAPSE)
        });
    }
}

fn has_uri(entry: &Extension, uri: &str) -> bool {
    entry
        .uri
        .as_deref()
        .is_some_and(|value| value.eq_ignore_ascii_case(uri))
}

fn whitespace(text: &str) -> bool {
    text.chars().all(|c| matches!(c, ' ' | '\t' | '\r' | '\n'))
}

/// The flags of an extension that holds exactly the known flag elements, each
/// with an explicit `val` of `0` or `1`, in schema order.
fn read_flags(
    entry: &Extension,
    namespace: &str,
    root: &str,
    names: &[&str],
) -> Option<Vec<Option<bool>>> {
    if !entry.attributes.is_empty() {
        return None;
    }
    let mut elements = Vec::new();
    for node in &entry.nodes {
        match node {
            ExtensionNode::Unknown(element) => elements.push(element),
            ExtensionNode::Text(text) if whitespace(text) => (),
            _ => return None,
        }
    }
    let [element] = elements.as_slice() else {
        return None;
    };
    if !element.name.is(namespace, root)
        || !element.attributes.is_empty()
        || !element.namespaces.is_empty()
    {
        return None;
    }
    let mut flags = vec![None; names.len()];
    let mut last = None;
    let mut tokens = element.children.iter().peekable();
    while let Some(token) = tokens.next() {
        match token {
            OpaqueToken::Text(text) if whitespace(text) => (),
            OpaqueToken::Start(start) => {
                let position = names
                    .iter()
                    .position(|name| start.name.is(namespace, name))?;
                if last.is_some_and(|last| position <= last)
                    || !start.namespaces.is_empty()
                    || start.attributes.len() != 1
                {
                    return None;
                }
                let (attribute, value) = &start.attributes[0];
                if !attribute.namespace_uri.is_empty() || attribute.local_name != "val" {
                    return None;
                }
                flags[position] = Some(match value.as_str() {
                    "1" => true,
                    "0" => false,
                    _ => return None,
                });
                last = Some(position);
                if !matches!(tokens.next(), Some(OpaqueToken::End)) {
                    return None;
                }
            }
            _ => return None,
        }
    }
    flags.iter().any(Option::is_some).then_some(flags)
}

fn build(
    uri: &str,
    (prefix, namespace): (&str, &str),
    root: &str,
    names: &[&str],
    flags: &[Option<bool>],
) -> Extension {
    let qname = |namespace_uri: &str, local: &str, prefix: Option<&str>| QName {
        namespace_uri:    namespace_uri.into(),
        local_name:       local.into(),
        preferred_prefix: prefix.map(Into::into),
    };
    let mut element = OpaqueElement {
        name:       qname(namespace, root, Some(prefix)),
        namespaces: Vec::new(),
        attributes: Vec::new(),
        children:   Vec::new(),
    };
    for (name, flag) in names.iter().zip(flags) {
        if let Some(flag) = flag {
            element
                .children
                .push(OpaqueToken::Start(Box::new(OpaqueStart {
                    name:       qname(namespace, name, Some(prefix)),
                    namespaces: Vec::new(),
                    attributes: vec![(
                        qname("", "val", None),
                        if *flag { "1" } else { "0" }.into(),
                    )],
                })));
            element.children.push(OpaqueToken::End);
        }
    }
    Extension::with_payload(
        uri,
        vec![
            (Some("c".into()), NS_C.into()),
            (Some(prefix.into()), namespace.into()),
        ],
        qname(NS_C, "ext", Some("c")),
        element,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flag_setters_follow_excels_present_when_on_form() {
        let mut options = PivotChartOptions::default();
        assert!(options.is_empty());
        options
            .set_show_axis_buttons(true)
            .set_show_expand_collapse_buttons(true);
        assert!(options.show_axis_buttons() && options.show_expand_collapse_buttons());
        assert!(!options.show_value_buttons());
        options.set_show_axis_buttons(false);
        assert!(!options.show_axis_buttons());
        assert!(!options.is_empty());
    }
}
