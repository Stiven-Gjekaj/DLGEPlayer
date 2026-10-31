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
