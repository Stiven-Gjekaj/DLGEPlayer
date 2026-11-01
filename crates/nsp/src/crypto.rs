//! The AES modes of the Switch: ECB for keys, XTS for the NCA header, and
//! CTR for the NCA sections.

use aes::Aes128;
use aes::cipher::generic_array::GenericArray;
use aes::cipher::{BlockDecrypt, BlockEncrypt, KeyInit};

/// The number of blocks that the CTR mode encrypts in one call.
const BATCH: usize = 64;

pub fn cipher(key: &[u8; 16]) -> Aes128 {
    Aes128::new(GenericArray::from_slice(key))
}

/// Decrypts 16-byte blocks in ECB mode.
pub fn ecb_decrypt(key: &[u8; 16], data: &mut [u8]) {
    let aes = cipher(key);
    for block in data.as_chunks_mut::<16>().0 {
        aes.decrypt_block(GenericArray::from_mut_slice(block));
    }
}

/// Encrypts 16-byte blocks in ECB mode. The tests use it.
#[cfg(test)]
pub fn ecb_encrypt(key: &[u8; 16], data: &mut [u8]) {
    let aes = cipher(key);
    for block in data.as_chunks_mut::<16>().0 {
        aes.encrypt_block(GenericArray::from_mut_slice(block));
    }
}

/// Multiplies an XTS tweak by x in GF(2^128), little-endian.
fn next_tweak(tweak: &mut [u8; 16]) {
    let mut carry = 0u8;
    for b in tweak.iter_mut() {
        let next = *b >> 7;
        *b = (*b << 1) | carry;
        carry = next;
    }
    if carry != 0 {
        tweak[0] ^= 0x87;
    }
}

/// Applies XTS to sectors. The Switch puts the sector number in the tweak
/// as a big-endian u128, not as a little-endian one.
fn xts(key: &[u8; 32], data: &mut [u8], sector_size: usize, first_sector: u128, encrypt: bool) {
    let data_key = cipher(key[..16].try_into().unwrap());
    let tweak_key = cipher(key[16..].try_into().unwrap());
    for (i, sector) in data.chunks_mut(sector_size).enumerate() {
        let mut tweak = (first_sector + i as u128).to_be_bytes();
        tweak_key.encrypt_block(GenericArray::from_mut_slice(&mut tweak));
        for block in sector.as_chunks_mut::<16>().0 {
            block.iter_mut().zip(&tweak).for_each(|(b, t)| *b ^= t);
            let ga = GenericArray::from_mut_slice(block);
            if encrypt {
                data_key.encrypt_block(ga);
            } else {
                data_key.decrypt_block(ga);
            }
            block.iter_mut().zip(&tweak).for_each(|(b, t)| *b ^= t);
            next_tweak(&mut tweak);
        }
    }
}

pub fn xts_decrypt(key: &[u8; 32], data: &mut [u8], sector_size: usize, first_sector: u128) {
    xts(key, data, sector_size, first_sector, false);
}

#[cfg(test)]
pub fn xts_encrypt(key: &[u8; 32], data: &mut [u8], sector_size: usize, first_sector: u128) {
    xts(key, data, sector_size, first_sector, true);
}

/// Applies CTR mode to data that starts at a 16-byte boundary. The counter
/// of a block is the 8 high bytes, then the block number as a big-endian
/// u64. Encryption and decryption are the same.
pub fn ctr(aes: &Aes128, high: &[u8; 8], first_block: u64, data: &mut [u8]) {
    let mut stream = [GenericArray::default(); BATCH];
    for (batch_index, batch) in data.chunks_mut(16 * BATCH).enumerate() {
        let blocks = batch.len().div_ceil(16);
        for (i, counter) in stream.iter_mut().take(blocks).enumerate() {
            let n = first_block + (batch_index * BATCH + i) as u64;
            counter[..8].copy_from_slice(high);
            counter[8..].copy_from_slice(&n.to_be_bytes());
        }
        aes.encrypt_blocks(&mut stream[..blocks]);
        for (b, s) in batch.iter_mut().zip(stream.iter().flatten()) {
            *b ^= s;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xts_matches_the_standard_vector() {
        // IEEE 1619 vector 1: keys and data of zeros, sector 0. The tweak
        // of sector 0 is zero in both byte orders.
        let mut data = [0u8; 32];
        xts_encrypt(&[0; 32], &mut data, 32, 0);
        assert_eq!(
            data[..16],
            [
                0x91, 0x7c, 0xf6, 0x9e, 0xbd, 0x68, 0xb2, 0xec, 0x9b, 0x9f, 0xe9, 0xa3, 0xea, 0xdd,
                0xa6, 0x92
            ]
        );
        assert_eq!(
            data[16..],
            [
                0xcd, 0x43, 0xd2, 0xf5, 0x95, 0x98, 0xed, 0x85, 0x8c, 0x02, 0xc2, 0x65, 0x2f, 0xbf,
                0x92, 0x2e
            ]
        );
        xts_decrypt(&[0; 32], &mut data, 32, 0);
        assert_eq!(data, [0; 32]);
    }

    #[test]
    fn xts_puts_the_sector_number_in_big_endian() {
        let key: [u8; 32] = std::array::from_fn(|i| i as u8);
        let mut a = [7u8; 0x400];
        xts_encrypt(&key, &mut a, 0x200, 5);
        // The second sector of a run from 5 is the first sector of a run from 6.
        let mut b = [7u8; 0x200];
        xts_encrypt(&key, &mut b, 0x200, 6);
        assert_eq!(a[0x200..], b);
        // The tweak of sector 1 is 1 in the last byte.
        let mut tweak = 1u128.to_be_bytes();
        cipher(key[16..].try_into().unwrap())
            .encrypt_block(GenericArray::from_mut_slice(&mut tweak));
        let mut block = [7u8; 16];
        block.iter_mut().zip(&tweak).for_each(|(b, t)| *b ^= t);
        cipher(key[..16].try_into().unwrap())
            .encrypt_block(GenericArray::from_mut_slice(&mut block));
        block.iter_mut().zip(&tweak).for_each(|(b, t)| *b ^= t);
        let mut c = [7u8; 0x200];
        xts_encrypt(&key, &mut c, 0x200, 1);
        assert_eq!(c[..16], block);
    }

    #[test]
    fn ctr_continues_at_any_block() {
        let aes = cipher(&[3; 16]);
        let high = [1, 2, 3, 4, 5, 6, 7, 8];
        let mut all = vec![0u8; 16 * 200];
        ctr(&aes, &high, 10, &mut all);
        let mut part = vec![0u8; 16 * 5];
        ctr(&aes, &high, 10 + 130, &mut part);
        assert_eq!(all[16 * 130..16 * 135], part[..]);
        // The keystream of block n is the encrypted counter.
        let mut counter = [0u8; 16];
        counter[..8].copy_from_slice(&high);
        counter[8..].copy_from_slice(&10u64.to_be_bytes());
        aes.encrypt_block(GenericArray::from_mut_slice(&mut counter));
        assert_eq!(all[..16], counter);
    }

    #[test]
    fn ecb_round_trips() {
        let mut data = *b"0123456789abcdef0123456789abcdef";
        ecb_encrypt(&[9; 16], &mut data);
        assert_ne!(&data, b"0123456789abcdef0123456789abcdef");
        ecb_decrypt(&[9; 16], &mut data);
        assert_eq!(&data, b"0123456789abcdef0123456789abcdef");
    }
}
