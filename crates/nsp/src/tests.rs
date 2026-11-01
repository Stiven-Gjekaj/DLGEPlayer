//! Opens NSP files that the tests build with test keys.

use std::io::{Read, Seek, SeekFrom};
use std::sync::Arc;

use crate::{Error, Game, Keys, crypto, nca, pfs0, romfs};

const HEADER_KEY: [u8; 32] = [0x11; 32];
const KEK: [u8; 16] = [0x22; 16];
const CTR_KEY: [u8; 16] = [0x33; 16];
const TITLEKEK: [u8; 16] = [0x44; 16];

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The NCA of `nca::build` has key generation 11, so the revision is 0a.
fn keys() -> Keys {
    Keys::parse(&format!(
        "header_key = {}\nkey_area_key_application_0a = {}\ntitlekek_0a = {}\n",
        hex(&HEADER_KEY),
        hex(&KEK),
        hex(&TITLEKEK)
    ))
}

fn files() -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("boot.lua", b"-- boot".to_vec()),
        (
            "Archive/title.nxonnx32.bea",
            (0..5000).map(|i| i as u8).collect(),
        ),
        ("Archive/sub/deep.bin", b"deep".to_vec()),
        ("movie/op.mp4", vec![9; 33]),
    ]
}

fn romfs_bytes() -> Vec<u8> {
    let files = files();
    let refs: Vec<(&str, &[u8])> = files.iter().map(|(p, d)| (*p, d.as_slice())).collect();
    romfs::build(&refs)
}

fn nsp(extra: &[(&str, &[u8])], rights_id: [u8; 16]) -> Vec<u8> {
    let nca = nca::build(&HEADER_KEY, &KEK, &CTR_KEY, rights_id, &romfs_bytes());
    let mut entries: Vec<(&str, &[u8])> = vec![("0123.nca", &nca)];
    entries.extend_from_slice(extra);
    pfs0::build(&entries)
}

#[test]
fn reads_files_through_the_key_area() {
    let game = Game::open(Arc::new(nsp(&[], [0; 16])), &keys()).unwrap();
    assert_eq!(game.program_id, 0x0100_bd70_0e64_8000);
    for (path, data) in files() {
        let file = game.romfs.file(path).unwrap_or_else(|| panic!("no {path}"));
        assert_eq!(file.read_all().unwrap(), data, "{path}");
    }
    assert_eq!(game.romfs.files().count(), 4);
    assert_eq!(game.romfs.list("Archive"), ["title.nxonnx32.bea"]);
    assert_eq!(game.romfs.list(""), ["boot.lua"]);
    assert!(game.romfs.is_dir("Archive/sub"));
    assert!(!game.romfs.is_dir("Archive/none"));
    assert!(game.romfs.file("none").is_none());
}

#[test]
fn reads_a_file_at_any_offset() {
    let game = Game::open(Arc::new(nsp(&[], [0; 16])), &keys()).unwrap();
    let mut reader = game
        .romfs
        .file("Archive/title.nxonnx32.bea")
        .unwrap()
        .reader();
    reader.seek(SeekFrom::Start(4093)).unwrap();
    let mut buf = [0u8; 7];
    reader.read_exact(&mut buf).unwrap();
    let expected: Vec<u8> = (4093..4100).map(|i| i as u8).collect();
    assert_eq!(buf[..], expected[..]);
    reader.seek(SeekFrom::End(-1)).unwrap();
    let mut rest = Vec::new();
    reader.read_to_end(&mut rest).unwrap();
    assert_eq!(rest, [(4999 % 256) as u8]);
}

#[test]
fn reads_files_through_a_ticket() {
    let rights_id = [0xab; 16];
    let mut ticket = vec![0u8; 0x2c0];
    ticket[..4].copy_from_slice(&0x10004u32.to_le_bytes());
    let mut title_key = CTR_KEY;
    crypto::ecb_encrypt(&TITLEKEK, &mut title_key);
    ticket[0x180..0x190].copy_from_slice(&title_key);
    let name = format!("{}.tik", hex(&rights_id));
    let bytes = nsp(&[(&name, &ticket)], rights_id);
    let game = Game::open(Arc::new(bytes), &keys()).unwrap();
    assert_eq!(
        game.romfs.file("boot.lua").unwrap().read_all().unwrap(),
        b"-- boot"
    );

    // With no ticket, the error names the ticket.
    let bytes = nsp(&[], rights_id);
    assert!(matches!(
        Game::open(Arc::new(bytes), &keys()),
        Err(Error::MissingKey(name)) if name.contains("tik")
    ));
}

#[test]
fn reports_wrong_keys() {
    let wrong = Keys::parse(&format!("header_key = {}", hex(&[0x99; 32])));
    assert!(matches!(
        Game::open(Arc::new(nsp(&[], [0; 16])), &wrong),
        Err(Error::BadMagic("NCA header"))
    ));
    let no_kek = Keys::parse(&format!("header_key = {}", hex(&HEADER_KEY)));
    assert_eq!(
        Game::open(Arc::new(nsp(&[], [0; 16])), &no_kek).err(),
        Some(Error::MissingKey("key_area_key_application_0a".into()))
    );
}

#[test]
fn reports_a_package_with_no_program() {
    let bytes = pfs0::build(&[("a.cnmt.nca", b"meta")]);
    assert_eq!(
        Game::open(Arc::new(bytes), &keys()).err(),
        Some(Error::NoProgram)
    );
}
