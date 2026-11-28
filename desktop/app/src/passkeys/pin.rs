// SPDX-License-Identifier: AGPL-3.0-or-later

//! CTAP2's PIN/UV auth protocols 1 and 2: the shared secret agreed with the key, and what it
//! encrypts and signs.

use aes::Aes256;
use aes::cipher::generic_array::GenericArray;
use aes::cipher::{BlockDecrypt, BlockEncrypt, KeyInit};
use ring::rand::{SecureRandom, SystemRandom};
use ring::{agreement, digest, hkdf, hmac};

use super::cbor::{self, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    One = 1,
    Two = 2,
}

/// The secret shared with the key for one exchange; only its encryption key is needed to get a
/// PIN token.
pub struct Shared {
    pub protocol: Protocol,
    aes: Vec<u8>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Failed;

/// Agrees on a secret with the key's `peer` COSE key, returning ours to send back.
pub fn agree(protocol: Protocol, peer: &Value) -> Result<(Value, Shared), Failed> {
    let (x, y) = (peer.get(-2).and_then(Value::as_bytes), peer.get(-3).and_then(Value::as_bytes));
    let (Some(x), Some(y)) = (x, y) else { return Err(Failed) };
    if x.len() != 32 || y.len() != 32 {
        return Err(Failed);
    }
    let peer = agreement::UnparsedPublicKey::new(&agreement::ECDH_P256, [&[4], x, y].concat());
    let rng = SystemRandom::new();
    let private = agreement::EphemeralPrivateKey::generate(&agreement::ECDH_P256, &rng).map_err(|_| Failed)?;
    let public = private.compute_public_key().map_err(|_| Failed)?;
    let z = agreement::agree_ephemeral(private, &peer, |z| z.to_vec()).map_err(|_| Failed)?;
    let ours = cbor::map([
        (1, Value::Int(2)),
        (3, Value::Int(-25)),
        (-1, Value::Int(1)),
        (-2, Value::Bytes(public.as_ref()[1..33].to_vec())),
        (-3, Value::Bytes(public.as_ref()[33..65].to_vec())),
    ]);
    Ok((ours, Shared::from_z(protocol, &z)?))
}

impl Shared {
    fn from_z(protocol: Protocol, z: &[u8]) -> Result<Self, Failed> {
        let aes = match protocol {
            Protocol::One => digest::digest(&digest::SHA256, z).as_ref().to_vec(),
            Protocol::Two => {
                let prk = hkdf::Salt::new(hkdf::HKDF_SHA256, &[0; 32]).extract(z);
                let mut aes = vec![0; 32];
                prk.expand(&[b"CTAP2 AES key"], hkdf::HKDF_SHA256)
                    .and_then(|okm| okm.fill(&mut aes))
                    .map_err(|_| Failed)?;
                aes
            },
        };
        Ok(Self { protocol, aes })
    }

    pub fn encrypt(&self, data: &[u8]) -> Vec<u8> {
        match self.protocol {
            Protocol::One => cbc_encrypt(&self.aes, &[0; 16], data),
            Protocol::Two => {
                let mut iv = [0; 16];
                let _ = SystemRandom::new().fill(&mut iv);
                [&iv[..], &cbc_encrypt(&self.aes, &iv, data)].concat()
            },
        }
    }

    pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>, Failed> {
        match self.protocol {
            Protocol::One => cbc_decrypt(&self.aes, &[0; 16], data),
            Protocol::Two if data.len() >= 16 => {
                cbc_decrypt(&self.aes, data[..16].try_into().map_err(|_| Failed)?, &data[16..])
            },
            Protocol::Two => Err(Failed),
        }
    }
}

/// Signs `data` with `key`, as much of the MAC as `protocol` sends.
pub fn authenticate(protocol: Protocol, key: &[u8], data: &[u8]) -> Vec<u8> {
    let tag = hmac::sign(&hmac::Key::new(hmac::HMAC_SHA256, key), data);
    match protocol {
        Protocol::One => tag.as_ref()[..16].to_vec(),
        Protocol::Two => tag.as_ref().to_vec(),
    }
}

/// What the key compares with its own PIN hash: the first 16 bytes of the PIN's SHA-256.
pub fn pin_hash(pin: &str) -> Vec<u8> {
    digest::digest(&digest::SHA256, pin.as_bytes()).as_ref()[..16].to_vec()
}

fn cbc_encrypt(key: &[u8], iv: &[u8; 16], data: &[u8]) -> Vec<u8> {
    let cipher = Aes256::new(GenericArray::from_slice(key));
    let mut previous = *iv;
    let mut out = Vec::with_capacity(data.len());
    for chunk in data.chunks(16) {
        let mut block = [0; 16];
        block[..chunk.len()].copy_from_slice(chunk);
        for (b, p) in block.iter_mut().zip(previous) {
            *b ^= p;
        }
        let mut block = GenericArray::from(block);
        cipher.encrypt_block(&mut block);
        previous = block.into();
        out.extend_from_slice(&previous);
    }
    out
}

fn cbc_decrypt(key: &[u8], iv: &[u8; 16], data: &[u8]) -> Result<Vec<u8>, Failed> {
    if data.is_empty() || !data.len().is_multiple_of(16) || key.len() != 32 {
        return Err(Failed);
    }
    let cipher = Aes256::new(GenericArray::from_slice(key));
    let mut previous = *iv;
    let mut out = Vec::with_capacity(data.len());
    for chunk in data.chunks(16) {
        let mut block = GenericArray::clone_from_slice(chunk);
        cipher.decrypt_block(&mut block);
        out.extend(block.iter().zip(previous).map(|(b, p)| b ^ p));
        previous.copy_from_slice(chunk);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
    }

    #[test]
    fn matches_the_aes_256_cbc_test_vector() {
        let key = hex("603deb1015ca71be2b73aef0857d77811f352c073b6108d72d9810a30914dff4");
        let iv: [u8; 16] = hex("000102030405060708090a0b0c0d0e0f").try_into().unwrap();
        let plain = hex("6bc1bee22e409f96e93d7e117393172aae2d8a571e03ac9c9eb76fac45af8e51");
        let cipher = hex("f58c4c04d6e5f1ba779eabfb5f7bfbd69cfc4e967edb808d679f777bc6702c7d");
        assert_eq!(cbc_encrypt(&key, &iv, &plain), cipher);
        assert_eq!(cbc_decrypt(&key, &iv, &cipher).unwrap(), plain);
    }

    #[test]
    fn round_trips_both_protocols() {
        for protocol in [Protocol::One, Protocol::Two] {
            let shared = Shared::from_z(protocol, &[7; 32]).unwrap();
            let token = [9; 32];
            let sealed = shared.encrypt(&token);
            assert_eq!(sealed.len(), if protocol == Protocol::One { 32 } else { 48 });
            assert_eq!(shared.decrypt(&sealed).unwrap(), token);
            assert_eq!(authenticate(protocol, &token, b"x").len(), if protocol == Protocol::One { 16 } else { 32 });
        }
    }

    #[test]
    fn agrees_with_the_other_side() {
        let rng = SystemRandom::new();
        let theirs = agreement::EphemeralPrivateKey::generate(&agreement::ECDH_P256, &rng).unwrap();
        let public = theirs.compute_public_key().unwrap();
        let cose = cbor::map([(-2, Value::from(&public.as_ref()[1..33])), (-3, Value::from(&public.as_ref()[33..]))]);
        let (ours, shared) = agree(Protocol::Two, &cose).unwrap();
        let ours =
            [&[4][..], ours.get(-2).unwrap().as_bytes().unwrap(), ours.get(-3).unwrap().as_bytes().unwrap()].concat();
        let z =
            agreement::agree_ephemeral(theirs, &agreement::UnparsedPublicKey::new(&agreement::ECDH_P256, ours), |z| {
                z.to_vec()
            })
            .unwrap();
        let other = Shared::from_z(Protocol::Two, &z).unwrap();
        assert_eq!(other.decrypt(&shared.encrypt(b"0123456789abcdef")).unwrap(), b"0123456789abcdef");
    }

    #[test]
    fn rejects_a_bad_peer_key() {
        assert!(agree(Protocol::One, &cbor::map([(-2, Value::Bytes(vec![0; 32]))])).is_err());
        let off_curve = cbor::map([(-2, Value::Bytes(vec![1; 32])), (-3, Value::Bytes(vec![1; 32]))]);
        assert!(agree(Protocol::One, &off_curve).is_err());
    }
}
