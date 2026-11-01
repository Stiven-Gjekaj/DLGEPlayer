//! Reads the RomFS of a real NSP and compares it with an extracted RomFS.
//!
//! The repository holds no game data and no keys. Set `DLGE_NSP` to the
//! NSP file, `DLGE_KEYS` to the keys file, and `DLGE_ROMFS` to a RomFS
//! that another tool extracted. Without the variables, the test does
//! nothing.

use std::io::Read;
use std::path::PathBuf;
use std::sync::Arc;

#[test]
fn romfs_matches_the_extracted_files() {
    let vars =
        ["DLGE_NSP", "DLGE_KEYS", "DLGE_ROMFS"].map(|v| std::env::var_os(v).map(PathBuf::from));
    let [Some(nsp), Some(keys), Some(romfs)] = vars else {
        eprintln!("DLGE_NSP, DLGE_KEYS, or DLGE_ROMFS is not set. The test does nothing.");
        return;
    };
    let keys = nsp::Keys::parse(&std::fs::read_to_string(keys).unwrap());
    let game = nsp::Game::open(Arc::new(std::fs::File::open(nsp).unwrap()), &keys).unwrap();

    let mut on_disk = Vec::new();
    let mut stack = vec![romfs.clone()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let relative = path.strip_prefix(&romfs).unwrap();
                on_disk.push(relative.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    on_disk.sort();
    let in_nsp: Vec<String> = game.romfs.files().map(|(p, _)| p.to_string()).collect();
    assert_eq!(in_nsp, on_disk, "the file lists differ");

    let (mut bytes, mut chunk_a, mut chunk_b) = (0u64, vec![0u8; 1 << 20], vec![0u8; 1 << 20]);
    for path in &in_nsp {
        let mut a = game.romfs.file(path).unwrap().reader();
        let mut b = std::io::BufReader::new(std::fs::File::open(romfs.join(path)).unwrap());
        loop {
            let n = a.read(&mut chunk_a).unwrap();
            b.read_exact(&mut chunk_b[..n]).unwrap();
            assert!(
                chunk_a[..n] == chunk_b[..n],
                "{path} differs near byte {bytes}"
            );
            bytes += n as u64;
            if n == 0 {
                break;
            }
        }
        assert_eq!(b.read(&mut chunk_b).unwrap(), 0, "{path} is longer on disk");
    }
    eprintln!("{} files and {bytes} bytes are the same", in_nsp.len());
}
