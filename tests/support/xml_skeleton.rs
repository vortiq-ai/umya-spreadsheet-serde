//! A structural view of chart XML for comparing the parts Excel wrote with the
//! parts the library writes: one line per element (depth, name, sorted
//! attributes without namespace declarations, text), source caches left out.
#![allow(dead_code)]
use std::io::{
    Cursor,
    Read,
};

pub fn chart_part(bytes: &[u8]) -> String {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let name = (0..archive.len())
        .map(|i| archive.by_index(i).unwrap().name().unwrap().into_owned())
        .find(|name| name.starts_with("xl/charts/chart") && name.ends_with(".xml"))
        .expect("a chart part");
    let mut xml = String::new();
    archive
        .by_name(&name)
        .unwrap()
        .read_to_string(&mut xml)
        .unwrap();
    xml
}

pub fn element<'a>(xml: &'a str, name: &str) -> &'a str {
    let open = format!("<{name}>");
    let close = format!("</{name}>");
    let start = xml
        .find(&open)
        .or_else(|| xml.find(&format!("<{name} ")))
        .unwrap_or_else(|| panic!("no {name} in {xml}"));
    let end = xml[start..].find(&close).unwrap() + close.len();
    &xml[start..start + end]
}

fn attributes(tag: &str) -> String {
    let mut found = Vec::new();
    let mut rest = tag;
    while let Some(eq) = rest.find("=\"") {
        let key = rest[..eq]
            .rsplit(|c: char| c.is_whitespace())
            .next()
            .unwrap();
        let value_start = eq + 2;
        let value_end = rest[value_start..].find('"').unwrap();
        if !key.starts_with("xmlns") {
            found.push(format!(
                "{key}={}",
                &rest[value_start..value_start + value_end]
            ));
        }
        rest = &rest[value_start + value_end + 1..];
    }
    found.sort();
    found.join(" ")
}

/// Lines `depth name attrs [text]` for every element, caches skipped.
pub fn skeleton(xml: &str) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut depth = 0usize;
    let mut skip_depth: Option<usize> = None;
    let mut pending_text = String::new();
    let mut rest = xml;
    while let Some(open) = rest.find('<') {
        let text = rest[..open].trim();
        if !text.is_empty() && skip_depth.is_none() {
            pending_text.push_str(text);
        }
        let close = rest[open..].find('>').unwrap() + open;
        let tag = &rest[open + 1..close];
        rest = &rest[close + 1..];
        if tag.starts_with('?') || tag.starts_with('!') {
            continue;
        }
        if let Some(name) = tag.strip_prefix('/') {
            depth -= 1;
            if skip_depth == Some(depth) {
                skip_depth = None;
                continue;
            }
            if skip_depth.is_none() {
                if !pending_text.is_empty() {
                    if let Some(last) = lines.last_mut() {
                        last.push_str(&format!(" text={}", pending_text));
                    }
                    pending_text.clear();
                }
                let _ = name;
            }
            continue;
        }
        let empty = tag.ends_with('/');
        let body = tag.trim_end_matches('/');
        let name = body.split_whitespace().next().unwrap();
        if skip_depth.is_none() {
            if name == "c:numCache" || name == "c:strCache" || name == "c:multiLvlStrCache" {
                skip_depth = Some(depth);
            } else {
                lines.push(
                    format!("{}{name} {}", "  ".repeat(depth), attributes(body))
                        .trim_end()
                        .to_owned(),
                );
            }
        }
        if !empty {
            depth += 1;
        }
    }
    lines
}

/// Direct children of `<c:plotArea>`, each as its own skeleton.
pub fn plot_children(xml: &str) -> Vec<(String, Vec<String>)> {
    let plot = element(xml, "c:plotArea");
    let inner = &plot["<c:plotArea>".len()..plot.len() - "</c:plotArea>".len()];
    let mut children = Vec::new();
    let mut rest = inner;
    while let Some(open) = rest.find('<') {
        let close = rest[open..].find('>').unwrap() + open;
        let tag = &rest[open + 1..close];
        let name = tag
            .trim_end_matches('/')
            .split_whitespace()
            .next()
            .unwrap()
            .to_owned();
        let end = if tag.ends_with('/') {
            close + 1
        } else {
            let closing = format!("</{name}>");
            let mut level = 0usize;
            let mut cursor = open;
            loop {
                let next = rest[cursor..].find('<').unwrap() + cursor;
                let next_close = rest[next..].find('>').unwrap() + next;
                let t = &rest[next + 1..next_close];
                if t.starts_with('/') {
                    level -= 1;
                    if level == 0 {
                        assert!(rest[next..].starts_with(&closing));
                        break next_close + 1;
                    }
                } else if !t.ends_with('/') && !t.starts_with('?') {
                    level += 1;
                }
                cursor = next_close + 1;
            }
        };
        let piece = &rest[open..end];
        children.push((name, skeleton(piece)));
        rest = &rest[end..];
    }
    children
}

const AXES: [&str; 4] = ["c:catAx", "c:dateAx", "c:valAx", "c:serAx"];

/// Groups in document order, then the axes as a sorted set (the library
/// writes them grouped by kind; the schema allows any order), then the rest.
pub fn normalized_plot(xml: &str) -> Vec<String> {
    let children = plot_children(xml);
    let mut lines = Vec::new();
    let mut axes = Vec::new();
    for (name, body) in children {
        if AXES.contains(&name.as_str()) {
            // `c:numFmt` of category and series axes belongs to the axis
            // element work, not to this comparison.
            let drop_numfmt = name == "c:catAx" || name == "c:serAx";
            let kept: Vec<_> = body
                .into_iter()
                .filter(|line| !(drop_numfmt && line.starts_with("  c:numFmt")))
                // Excel leaves the default `c:crosses val=autoZero` out of a
                // deleted secondary category axis; the library writes it.
                .filter(|line| line != "  c:crosses val=autoZero")
                .collect();
            axes.push(kept.join("\n"));
        } else {
            // The library writes the defaults `c:overlap val=0` of a bar group
            // and `c:marker val=0` of a line group that Excel leaves out.
            lines.extend(
                body.into_iter()
                    .filter(|line| line != "  c:overlap val=0" && line != "  c:marker val=0"),
            );
        }
    }
    axes.sort();
    lines.push("--axes--".into());
    lines.extend(axes);
    lines
}
