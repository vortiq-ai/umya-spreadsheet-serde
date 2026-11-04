use std::io::{
    self,
    Read,
};

use crate::{
    XlsxError,
    helper::const_str::{
        CONTENT_TYPES,
        CONTYPES_NS,
    },
    structs::{
        Workbook,
        content_types::PackageContentTypes,
    },
    xml::{
        attributes,
        direct_children,
        no_children,
        required_attribute,
        xml_error,
    },
};

pub(crate) fn read<R: Read + io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    workbook: &mut Workbook,
) -> Result<(), XlsxError> {
    let mut bytes = Vec::new();
    super::driver::zip_by_name(archive, CONTENT_TYPES)?.read_to_end(&mut bytes)?;
    workbook.set_package_content_types(parse(&bytes).map_err(xml_error)?);
    Ok(())
}

fn parse(bytes: &[u8]) -> Result<PackageContentTypes, String> {
    let root = crate::xml::root(bytes)?;
    if !root.name.is(CONTYPES_NS, "Types") {
        return Err("invalid package content-types root".into());
    }
    attributes(&root, &[], &[])?;
    let mut types = PackageContentTypes::default();
    for entry in direct_children(&root)? {
        no_children(&entry)?;
        if entry.name.is(CONTYPES_NS, "Override") {
            attributes(&entry, &["PartName", "ContentType"], &[])?;
            types.insert_override(
                required_attribute(&entry, "", "PartName")?.to_owned(),
                required_attribute(&entry, "", "ContentType")?.to_owned(),
            )?;
        } else if entry.name.is(CONTYPES_NS, "Default") {
            attributes(&entry, &["Extension", "ContentType"], &[])?;
            types.insert_default(
                required_attribute(&entry, "", "Extension")?.to_owned(),
                required_attribute(&entry, "", "ContentType")?.to_owned(),
            )?;
        } else {
            return Err("unsupported package content-type entry".into());
        }
    }
    Ok(types)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefixed_and_explicit_empty_entries_resolve_defaults_and_overrides() {
        let xml = format!(
            r#"<ct:Types xmlns:ct="{CONTYPES_NS}"><ct:Default Extension="XML" ContentType="application/xml"></ct:Default><ct:Override PartName="/xl/custom&amp;1.xml" ContentType="application/custom+xml"></ct:Override></ct:Types>"#
        );
        let types = parse(xml.as_bytes()).unwrap();
        assert_eq!(types.get("xl/custom&1.xml"), Some("application/custom+xml"));
        assert_eq!(types.get("xl/other.xml"), Some("application/xml"));
    }

    #[test]
    fn identical_entries_coalesce_and_conflicting_entries_fail() {
        for entry in [
            r#"<Default Extension="xml" ContentType="application/a"/>"#,
            r#"<Override PartName="/xl/a.xml" ContentType="application/a"/>"#,
        ] {
            let xml = format!(r#"<Types xmlns="{CONTYPES_NS}">{entry}{entry}</Types>"#);
            assert!(parse(xml.as_bytes()).is_ok());
            let conflicting = entry.replace(
                r#"ContentType="application/a""#,
                r#"ContentType="application/b""#,
            );
            let xml = format!(r#"<Types xmlns="{CONTYPES_NS}">{entry}{conflicting}</Types>"#);
            assert!(parse(xml.as_bytes()).is_err());
        }
    }

    #[test]
    fn invalid_content_types_are_rejected_after_decoding() {
        for content_type in [
            "",
            "application/xml ",
            "application/&#10;xml",
            "application/&#9;xml",
            "not-a-media-type",
        ] {
            let xml = format!(
                r#"<Types xmlns="{CONTYPES_NS}"><Default Extension="xml" ContentType="{content_type}"/></Types>"#
            );
            assert!(parse(xml.as_bytes()).is_err(), "accepted {content_type:?}");
        }
    }

    #[test]
    fn override_part_names_use_ascii_case_insensitive_identity() {
        let xml = format!(
            r#"<Types xmlns="{CONTYPES_NS}"><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/charts/Style1.xml" ContentType="application/vnd.example.chartstyle+xml"/></Types>"#
        );
        let types = parse(xml.as_bytes()).unwrap();
        assert_eq!(
            types.get("xl/charts/style1.xml"),
            Some("application/vnd.example.chartstyle+xml")
        );
        let conflicting = xml.replace("</Types>", r#"<Override PartName="/XL/CHARTS/STYLE1.XML" ContentType="application/vnd.example.other+xml"/></Types>"#);
        assert!(parse(conflicting.as_bytes()).is_err());
    }

    #[test]
    fn media_type_parameters_preserve_legal_whitespace_and_quoted_values() {
        for content_type in [
            "application/example;version=1",
            "application/example ; note=\"two words\"",
            "application/example\t;\tnote=\"a\tb\"",
            "application/example;note=\"\"",
            "application/example;note=\"a\\\"b\\\\c\"",
        ] {
            assert!(
                crate::structs::content_types::validate_content_type(content_type).is_ok(),
                "{content_type:?}"
            );
        }
        for content_type in [
            "application /example",
            "application/example; note =value",
            "application/example;",
            "application/example;note=\"unterminated",
            "application/example;note=\"bad\rvalue\"",
            "application/example;note=\"bad\nvalue\"",
            "application/example;note=\"bad\u{7f}value\"",
        ] {
            assert!(
                crate::structs::content_types::validate_content_type(content_type).is_err(),
                "{content_type:?}"
            );
        }
    }
}
