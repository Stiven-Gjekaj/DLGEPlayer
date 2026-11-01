//! Reads the header and the sections of an NCA file, version 3.
//!
//! The first 0xc00 bytes are the header. It is encrypted with AES-XTS and
//! the `header_key`, in sectors of 0x200 bytes. After decryption:
//!
//! | Offset | Value |
//! |---|---|
//! | 0x200 | the magic `NCA3` |
//! | 0x205 | the content type. 0 is a program. |
//! | 0x206 | the old key revision |
//! | 0x207 | the key area index: 0 application, 1 ocean, 2 system |
//! | 0x210 | the program ID, u64 |
//! | 0x220 | the new key revision. The larger revision is used. |
//! | 0x230 | the rights ID. Zero if the NCA uses the key area. |
//! | 0x240 | 4 section entries: the start and the end in units of 0x200 bytes |
//! | 0x300 | the key area: 4 keys, encrypted with AES-ECB. Key 2 is the CTR key. |
//! | 0x400 | 4 section headers of 0x200 bytes |
//!
//! A section header has the file system type at 0x02 (0 RomFS), the hash
//! type at 0x03 (3 IVFC), the encryption type at 0x04 (1 none, 3 CTR), the
//! IVFC hash data from 0x08, and the high 8 bytes of the CTR counter at
//! 0x140, in reverse order. The last IVFC level is the RomFS. The counter
//! of a block is those 8 bytes, then the offset in the NCA divided by 16,
//! as a big-endian u64.

use std::io;
use std::sync::Arc;

use aes::Aes128;

use crate::crypto;
use crate::{Error, ReadAt, Region, Result};

pub const PROGRAM: u8 = 0;
const HEADER_LEN: usize = 0xc00;
const SECTOR: usize = 0x200;
const MEDIA_UNIT: u64 = 0x200;
const ROMFS: u8 = 0;
const IVFC: u8 = 3;
const NO_ENCRYPTION: u8 = 1;
const CTR: u8 = 3;

/// One section of an NCA.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    /// The start and the end in the NCA.
    pub start: u64,
    pub end: u64,
    pub fs_type: u8,
    pub hash_type: u8,
    pub encryption: u8,
    /// The high 8 bytes of the CTR counter.
    pub counter: [u8; 8],
    /// The offset and the size of the last IVFC level, from the start of
    /// the section. It is the RomFS.
    pub data_offset: u64,
    pub data_size: u64,
}

/// The decrypted header of an NCA.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    pub content_type: u8,
    pub key_generation: u8,
    pub key_area_index: u8,
    pub program_id: u64,
    pub rights_id: [u8; 16],
    /// The encrypted key area.
    pub key_area: [u8; 64],
    pub sections: Vec<Section>,
}

impl Header {
    /// Reads and decrypts the header of an NCA.
    pub fn read(nca: &dyn ReadAt, keys: &crate::Keys) -> Result<Header> {
        let mut h = vec![0u8; HEADER_LEN];
        nca.read_at(&mut h, 0)?;
        crypto::xts_decrypt(&keys.header_key()?, &mut h, SECTOR, 0);
        match &h[0x200..0x204] {
            b"NCA3" => {}
            b"NCA2" | b"NCA0" => {
                return Err(Error::Unsupported("an NCA older than version 3".into()));
            }
            _ => return Err(Error::BadMagic("NCA header")),
        }
        let u32_at = |at: usize| u32::from_le_bytes(h[at..at + 4].try_into().unwrap());
        let u64_at = |at: usize| u64::from_le_bytes(h[at..at + 8].try_into().unwrap());
        let mut sections = Vec::new();
        for i in 0..4 {
            let entry = 0x240 + 0x10 * i;
            let (start, end) = (u32_at(entry) as u64, u32_at(entry + 4) as u64);
            if start == 0 && end == 0 {
                continue;
            }
            let fs = 0x400 + 0x200 * i;
            let levels = u32_at(fs + 0x14) as usize;
            let (data_offset, data_size) = if h[fs + 3] == IVFC && (2..=7).contains(&levels) {
                let level = fs + 0x18 + 0x18 * (levels - 2);
                (u64_at(level), u64_at(level + 8))
            } else {
                (0, 0)
            };
            let mut counter = [0u8; 8];
            for (j, c) in counter.iter_mut().enumerate() {
                *c = h[fs + 0x147 - j];
            }
            sections.push(Section {
                start: start * MEDIA_UNIT,
                end: end * MEDIA_UNIT,
                fs_type: h[fs + 2],
                hash_type: h[fs + 3],
                encryption: h[fs + 4],
                counter,
                data_offset,
                data_size,
            });
        }
        Ok(Header {
            content_type: h[0x205],
            key_generation: h[0x206].max(h[0x220]),
            key_area_index: h[0x207],
            program_id: u64_at(0x210),
            rights_id: h[0x230..0x240].try_into().unwrap(),
            key_area: h[0x300..0x340].try_into().unwrap(),
            sections,
        })
    }

    /// Gives the revision of the keys: the key generation less one.
    pub fn key_revision(&self) -> u8 {
        self.key_generation.saturating_sub(1)
    }

    /// Gives the first RomFS section.
    pub fn romfs_section(&self) -> Option<&Section> {
        self.sections
            .iter()
            .find(|s| s.fs_type == ROMFS && s.hash_type == IVFC && s.data_size > 0)
    }

    /// Decrypts the CTR key of the key area.
    pub fn key_area_key(&self, keys: &crate::Keys) -> Result<[u8; 16]> {
        let kek = keys.key_area_key(self.key_area_index, self.key_revision())?;
        let mut key: [u8; 16] = self.key_area[0x20..0x30].try_into().unwrap();
        crypto::ecb_decrypt(&kek, &mut key);
        Ok(key)
    }
}

/// Decrypts the title key of a common ticket.
pub fn title_key(ticket: &[u8], revision: u8, keys: &crate::Keys) -> Result<[u8; 16]> {
    let kind = u32::from_le_bytes(
        ticket
            .get(..4)
            .ok_or(Error::OutOfBounds("ticket"))?
            .try_into()
            .unwrap(),
    );
    // The signature size, with the padding to 0x40 bytes.
    let data = match kind {
        0x10000 | 0x10003 => 4 + 0x200 + 0x3c,
        0x10001 | 0x10004 => 4 + 0x100 + 0x3c,
        0x10002 | 0x10005 => 4 + 0x3c + 0x40,
        _ => return Err(Error::Unsupported(format!("ticket type {kind:#x}"))),
    };
    let block = ticket
        .get(data..data + 0x180)
        .ok_or(Error::OutOfBounds("ticket"))?;
    if block[0x101] != 0 {
        return Err(Error::Unsupported(
            "a personalized ticket. It needs the private key of the console.".into(),
        ));
    }
    let mut key: [u8; 16] = block[0x40..0x50].try_into().unwrap();
    crypto::ecb_decrypt(&keys.titlekek(revision)?, &mut key);
    Ok(key)
}

impl Section {
    /// Gives the decrypted data of the last IVFC level, the RomFS.
    pub fn data(&self, nca: Region, key: &[u8; 16]) -> Result<Region> {
        let start = self.start + self.data_offset;
        if start + self.data_size > self.end {
            return Err(Error::OutOfBounds("RomFS level"));
        }
        let inner: Arc<dyn ReadAt> = match self.encryption {
            NO_ENCRYPTION => Arc::new(nca),
            CTR => Arc::new(CtrReader {
                nca,
                aes: crypto::cipher(key),
                counter: self.counter,
            }),
            other => {
                return Err(Error::Unsupported(format!(
                    "section encryption type {other}"
                )));
            }
        };
        Ok(Region::new(inner, start, self.data_size))
    }
}

/// Decrypts an NCA in CTR mode while it reads. Offsets are from the start
/// of the NCA.
struct CtrReader {
    nca: Region,
    aes: Aes128,
    counter: [u8; 8],
}

impl ReadAt for CtrReader {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> io::Result<()> {
        if buf.is_empty() {
            return Ok(());
        }
        let first = offset / 16;
        let skip = (offset % 16) as usize;
        let end = (offset + buf.len() as u64).next_multiple_of(16);
        let mut data = vec![0u8; (end - first * 16) as usize];
        self.nca.read_at(&mut data, first * 16)?;
        crypto::ctr(&self.aes, &self.counter, first, &mut data);
        buf.copy_from_slice(&data[skip..skip + buf.len()]);
        Ok(())
    }
}

/// Builds an encrypted NCA with one RomFS section. The tests use it.
#[cfg(test)]
pub fn build(
    header_key: &[u8; 32],
    kek: &[u8; 16],
    ctr_key: &[u8; 16],
    rights_id: [u8; 16],
    romfs: &[u8],
) -> Vec<u8> {
    let data_offset = 0x200u64;
    let section_start = HEADER_LEN as u64;
    let section_len = (data_offset + romfs.len() as u64).next_multiple_of(MEDIA_UNIT);
    let mut h = vec![0u8; HEADER_LEN];
    h[0x200..0x204].copy_from_slice(b"NCA3");
    h[0x205] = PROGRAM;
    h[0x206] = 2;
    h[0x220] = 11;
    h[0x210..0x218].copy_from_slice(&0x0100_bd70_0e64_8000u64.to_le_bytes());
    h[0x230..0x240].copy_from_slice(&rights_id);
    let start = (section_start / MEDIA_UNIT) as u32;
    let end = ((section_start + section_len) / MEDIA_UNIT) as u32;
    h[0x240..0x244].copy_from_slice(&start.to_le_bytes());
    h[0x244..0x248].copy_from_slice(&end.to_le_bytes());
    let mut key_area = [0u8; 64];
    key_area[0x20..0x30].copy_from_slice(ctr_key);
    crypto::ecb_encrypt(kek, &mut key_area);
    h[0x300..0x340].copy_from_slice(&key_area);
    let fs = 0x400;
    h[fs + 2] = ROMFS;
    h[fs + 3] = IVFC;
    h[fs + 4] = CTR;
    h[fs + 8..fs + 12].copy_from_slice(b"IVFC");
    h[fs + 0x14..fs + 0x18].copy_from_slice(&7u32.to_le_bytes());
    let level = fs + 0x18 + 0x18 * 5;
    h[level..level + 8].copy_from_slice(&data_offset.to_le_bytes());
    h[level + 8..level + 16].copy_from_slice(&(romfs.len() as u64).to_le_bytes());
    let counter = [1u8, 2, 3, 4, 5, 6, 7, 8];
    for (j, c) in counter.iter().enumerate() {
        h[fs + 0x147 - j] = *c;
    }
    crypto::xts_encrypt(header_key, &mut h, SECTOR, 0);

    let mut section = vec![0u8; section_len as usize];
    section[data_offset as usize..data_offset as usize + romfs.len()].copy_from_slice(romfs);
    crypto::ctr(
        &crypto::cipher(ctr_key),
        &counter,
        section_start / 16,
        &mut section,
    );
    h.extend(section);
    h
}
