//! Checks the sprite rows of a real dump.
//!
//! The repository holds no game data. Set `DLGE_ARCHIVES` to the `Archive`
//! directory of a RomFS dump to run this test. Without the variable, the
//! test does nothing.

use std::path::PathBuf;

use importer::sprite::Sprite;

#[test]
fn every_sprite_row_fits_its_sheet() {
    let Some(dir) = std::env::var_os("DLGE_ARCHIVES").map(PathBuf::from) else {
        eprintln!("DLGE_ARCHIVES is not set. The corpus test does nothing.");
        return;
    };
    let bytes = std::fs::read(dir.join("static~chara.bin.nxonnx32.bea")).unwrap();
    let archive = bea::Archive::parse(&bytes).unwrap();
    let data = archive.read(&archive.entries[0]).unwrap();
    let table = bea::table::Table::parse(&data).unwrap();

    let mut frames = 0;
    for row in &table.rows {
        let sprite = Sprite::from_row(row).unwrap();
        // A blank sheet of the right size checks every rectangle.
        let sheet = vec![0u8; (sprite.sheet.x * sprite.sheet.y * 4) as usize];
        for e in 0..sprite.expressions.len() {
            for f in 0..3 {
                sprite.compose(&sheet, e, f, f).unwrap_or_else(|err| {
                    panic!("{} expression {e} frame {f}: {err}", sprite.name)
                });
                frames += 1;
            }
        }
    }
    eprintln!("{} sprites, {frames} frames fit", table.rows.len());
}

/// Resolves the second sprite name of each script call with two names.
/// It needs `DLGE_ARCHIVES` and `DLGE_SCRIPTS`.
#[test]
fn script_sprite_names_resolve() {
    let (Some(archives), Some(scripts)) = (
        std::env::var_os("DLGE_ARCHIVES").map(PathBuf::from),
        std::env::var_os("DLGE_SCRIPTS").map(PathBuf::from),
    ) else {
        eprintln!("DLGE_ARCHIVES or DLGE_SCRIPTS is not set. The test does nothing.");
        return;
    };
    let bytes = std::fs::read(archives.join("static~chara.bin.nxonnx32.bea")).unwrap();
    let archive = bea::Archive::parse(&bytes).unwrap();
    let table = bea::table::Table::parse(&archive.read(&archive.entries[0]).unwrap()).unwrap();
    let sprites = table
        .rows
        .iter()
        .map(|r| Sprite::from_row(r).unwrap())
        .collect();
    let catalog = importer::sprite::Catalog::new(sprites);

    use importer::sprite::ResolveError;
    use stcm2l::decode::{Instruction, Value};
    let (mut calls, mut resolved) = (0, 0);
    let mut missing_expressions = std::collections::BTreeMap::new();
    let mut unknown = Vec::new();
    for entry in std::fs::read_dir(&scripts).unwrap() {
        let path = entry.unwrap().path();
        if !path.extension().is_some_and(|e| e == "dat") {
            continue;
        }
        let script = stcm2l::Script::parse(&std::fs::read(&path).unwrap()).unwrap();
        for action in &script.actions {
            let Ok(Instruction::Call { args, .. }) = stcm2l::decode::decode(&script, action) else {
                continue;
            };
            let [Value::Text(_), Value::Text(name), ..] = args.as_slice() else {
                continue;
            };
            calls += 1;
            match catalog.resolve(name) {
                Ok(_) => resolved += 1,
                Err(ResolveError::UnknownExpression { expression, .. }) => {
                    *missing_expressions.entry(expression).or_insert(0) += 1
                }
                Err(err) => unknown.push(format!("{}: {err:?}", path.display())),
            }
        }
    }
    assert!(calls > 0, "no sprite calls in {}", scripts.display());
    // Every pose must resolve. An expression can be missing from a pose.
    assert!(
        unknown.is_empty(),
        "{} names failed:\n{}",
        unknown.len(),
        unknown.join("\n")
    );
    eprintln!("{resolved} of {calls} calls resolve; missing expressions: {missing_expressions:?}");
}
