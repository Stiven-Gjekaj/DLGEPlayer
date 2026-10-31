//! Reads each layout of a real dump.
//!
//! The repository holds no game data. Set `DLGE_ARCHIVES` to the `Archive`
//! directory of a RomFS dump to run this test. Without the variable, the
//! test does nothing.

use std::collections::BTreeSet;
use std::path::PathBuf;

use lyt::{Kind, Layout, Pane};

/// Checks that each index of a pane points into the lists of the layout.
fn check(layout: &Layout, pane: &Pane, parts: &BTreeSet<String>, errors: &mut Vec<String>) {
    let material = match &pane.kind {
        Kind::Picture { material, .. } | Kind::Text { material, .. } => Some(*material),
        Kind::Part { layout: part, .. } => {
            if !parts.contains(part) {
                errors.push(format!("{}: no layout {part}", pane.name));
            }
            None
        }
        _ => None,
    };
    if let Some(m) = material
        && m as usize >= layout.materials.len()
    {
        errors.push(format!(
            "{}: material {m} of {}",
            pane.name,
            layout.materials.len()
        ));
    }
    if let Kind::Text { font, .. } = &pane.kind
        && *font as usize >= layout.fonts.len()
    {
        errors.push(format!("{}: font {font}", pane.name));
    }
    for child in &pane.children {
        check(layout, child, parts, errors);
    }
}

#[test]
fn every_layout_reads() {
    let Some(dir) = std::env::var_os("DLGE_ARCHIVES").map(PathBuf::from) else {
        eprintln!("DLGE_ARCHIVES is not set. The corpus test does nothing.");
        return;
    };
    let (mut count, mut panes, mut texts) = (0, 0, 0);
    let mut errors = Vec::new();
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        let bytes = std::fs::read(&path).unwrap();
        let Ok(archive) = bea::Archive::parse(&bytes) else {
            continue;
        };
        for e in archive.entries.iter().filter(|e| e.name.ends_with(".lyt")) {
            let data = archive.read(e).unwrap();
            let files = sarc::parse(&data).unwrap();
            let parts: BTreeSet<String> = files
                .iter()
                .filter_map(|f| f.name.strip_suffix(".bflyt"))
                .filter_map(|n| n.rsplit('/').next())
                .map(str::to_string)
                .collect();
            for f in files.iter().filter(|f| f.name.ends_with(".bflyt")) {
                let where_ = format!("{}/{}", e.name, f.name);
                let layout = match lyt::parse(f.data) {
                    Ok(l) => l,
                    Err(err) => {
                        errors.push(format!("{where_}: {err}"));
                        continue;
                    }
                };
                count += 1;
                for m in &layout.materials {
                    for t in &m.textures {
                        if t.texture as usize >= layout.textures.len() {
                            errors.push(format!("{where_}: {} texture {}", m.name, t.texture));
                        }
                    }
                }
                let mut stack: Vec<&Pane> = layout.root.iter().collect();
                while let Some(p) = stack.pop() {
                    panes += 1;
                    texts += matches!(p.kind, Kind::Text { .. }) as usize;
                    stack.extend(&p.children);
                }
                let mut local = Vec::new();
                if let Some(root) = &layout.root {
                    check(&layout, root, &parts, &mut local);
                }
                errors.extend(local.into_iter().map(|m| format!("{where_}: {m}")));
            }
        }
    }
    assert!(count > 0, "no layouts in {}", dir.display());
    assert!(
        errors.is_empty(),
        "{} errors:\n{}",
        errors.len(),
        errors[..errors.len().min(40)].join("\n")
    );
    eprintln!("{count} layouts, {panes} panes, {texts} text panes");
}

#[test]
fn every_animation_reads() {
    let Some(dir) = std::env::var_os("DLGE_ARCHIVES").map(PathBuf::from) else {
        eprintln!("DLGE_ARCHIVES is not set. The corpus test does nothing.");
        return;
    };
    let (mut count, mut keys) = (0, 0);
    let mut errors = Vec::new();
    for entry in std::fs::read_dir(&dir).unwrap() {
        let bytes = std::fs::read(entry.unwrap().path()).unwrap();
        let Ok(archive) = bea::Archive::parse(&bytes) else {
            continue;
        };
        for e in archive.entries.iter().filter(|e| e.name.ends_with(".lyt")) {
            let data = archive.read(e).unwrap();
            for f in sarc::parse(&data).unwrap() {
                if !f.name.ends_with(".bflan") {
                    continue;
                }
                match lyt::anim::parse(f.data) {
                    Ok(a) => {
                        count += 1;
                        for t in &a.targets {
                            for tag in &t.tags {
                                for c in &tag.curves {
                                    keys += c.keys.len();
                                    // The keys of a curve come in the order of the frames.
                                    if c.keys.windows(2).any(|w| w[0].frame > w[1].frame) {
                                        errors.push(format!("{}/{}: {:?}", e.name, f.name, c.keys));
                                    }
                                }
                            }
                        }
                    }
                    Err(err) => errors.push(format!("{}/{}: {err}", e.name, f.name)),
                }
            }
        }
    }
    assert!(count > 0, "no animations in {}", dir.display());
    assert!(
        errors.is_empty(),
        "{} errors:\n{}",
        errors.len(),
        errors.join("\n")
    );
    eprintln!("{count} animations, {keys} keys");
}
