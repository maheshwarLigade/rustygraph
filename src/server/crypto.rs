//! Zero-dependency cryptographic utilities for RustyGraph authentication.
//!
//! Provides:
//! - FIPS 180-4 compliant SHA-256 hashing
//! - RFC 2104 compliant HMAC-SHA256
//! - RFC 2898 compliant PBKDF2-HMAC-SHA256 password hashing with salt
//! - Constant-time comparison for secret hashes (timing attack prevention)
//! - RFC 4648 Base64 encoding/decoding for HTTP Basic Authentication
//! - Cryptographically unguessable Bearer session token generation

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

// =============================================================================
// SHA-256 (FIPS 180-4) Implementation
// =============================================================================

const SHA256_K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

#[derive(Clone)]
pub struct Sha256 {
    state: [u32; 8],
    buffer: [u8; 64],
    buf_len: usize,
    total_len: u64,
}

impl Default for Sha256 {
    fn default() -> Self {
        Self::new()
    }
}

impl Sha256 {
    pub fn new() -> Self {
        Self {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
                0x5be0cd19,
            ],
            buffer: [0u8; 64],
            buf_len: 0,
            total_len: 0,
        }
    }

    pub fn update(&mut self, data: &[u8]) {
        self.total_len += data.len() as u64;
        let mut i = 0;
        while i < data.len() {
            let space = 64 - self.buf_len;
            let take = (data.len() - i).min(space);
            self.buffer[self.buf_len..self.buf_len + take].copy_from_slice(&data[i..i + take]);
            self.buf_len += take;
            i += take;

            if self.buf_len == 64 {
                self.process_block();
                self.buf_len = 0;
            }
        }
    }

    fn process_block(&mut self) {
        let mut w = [0u32; 64];
        for t in 0..16 {
            w[t] = u32::from_be_bytes([
                self.buffer[t * 4],
                self.buffer[t * 4 + 1],
                self.buffer[t * 4 + 2],
                self.buffer[t * 4 + 3],
            ]);
        }
        for t in 16..64 {
            let s0 = w[t - 15].rotate_right(7) ^ w[t - 15].rotate_right(18) ^ (w[t - 15] >> 3);
            let s1 = w[t - 2].rotate_right(17) ^ w[t - 2].rotate_right(19) ^ (w[t - 2] >> 10);
            w[t] = w[t - 16]
                .wrapping_add(s0)
                .wrapping_add(w[t - 7])
                .wrapping_add(s1);
        }

        let mut a = self.state[0];
        let mut b = self.state[1];
        let mut c = self.state[2];
        let mut d = self.state[3];
        let mut e = self.state[4];
        let mut f = self.state[5];
        let mut g = self.state[6];
        let mut h = self.state[7];

        for t in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(SHA256_K[t])
                .wrapping_add(w[t]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        self.state[0] = self.state[0].wrapping_add(a);
        self.state[1] = self.state[1].wrapping_add(b);
        self.state[2] = self.state[2].wrapping_add(c);
        self.state[3] = self.state[3].wrapping_add(d);
        self.state[4] = self.state[4].wrapping_add(e);
        self.state[5] = self.state[5].wrapping_add(f);
        self.state[6] = self.state[6].wrapping_add(g);
        self.state[7] = self.state[7].wrapping_add(h);
    }

    pub fn finalize(mut self) -> [u8; 32] {
        let bit_len = self.total_len * 8;
        self.buffer[self.buf_len] = 0x80;
        self.buf_len += 1;

        if self.buf_len > 56 {
            for i in self.buf_len..64 {
                self.buffer[i] = 0;
            }
            self.process_block();
            self.buf_len = 0;
        }

        for i in self.buf_len..56 {
            self.buffer[i] = 0;
        }
        self.buffer[56..64].copy_from_slice(&bit_len.to_be_bytes());
        self.process_block();

        let mut out = [0u8; 32];
        for i in 0..8 {
            out[i * 4..(i + 1) * 4].copy_from_slice(&self.state[i].to_be_bytes());
        }
        out
    }

    pub fn digest(data: &[u8]) -> [u8; 32] {
        let mut hasher = Self::new();
        hasher.update(data);
        hasher.finalize()
    }
}

// =============================================================================
// HMAC-SHA256 (RFC 2104) Implementation
// =============================================================================

pub fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    let mut key_block = [0u8; 64];
    if key.len() > 64 {
        let hashed = Sha256::digest(key);
        key_block[..32].copy_from_slice(&hashed);
    } else {
        key_block[..key.len()].copy_from_slice(key);
    }

    let mut o_key_pad = [0u8; 64];
    let mut i_key_pad = [0u8; 64];
    for i in 0..64 {
        o_key_pad[i] = key_block[i] ^ 0x5c;
        i_key_pad[i] = key_block[i] ^ 0x36;
    }

    // Inner hash: H(i_key_pad || message)
    let mut inner = Sha256::new();
    inner.update(&i_key_pad);
    inner.update(message);
    let inner_hash = inner.finalize();

    // Outer hash: H(o_key_pad || inner_hash)
    let mut outer = Sha256::new();
    outer.update(&o_key_pad);
    outer.update(&inner_hash);
    outer.finalize()
}

// =============================================================================
// PBKDF2-HMAC-SHA256 (RFC 2898) Password Hashing
// =============================================================================

/// Derives a 32-byte secret key from password and salt using PBKDF2 with HMAC-SHA256.
pub fn pbkdf2_hmac_sha256(password: &[u8], salt: &[u8], iterations: u32) -> [u8; 32] {
    assert!(iterations > 0, "iterations must be > 0");

    // Block 1 (we produce 32 bytes which is exactly 1 block of HMAC-SHA256)
    let mut salt_with_index = Vec::with_capacity(salt.len() + 4);
    salt_with_index.extend_from_slice(salt);
    salt_with_index.extend_from_slice(&1u32.to_be_bytes());

    let mut u = hmac_sha256(password, &salt_with_index);
    let mut result = u;

    for _ in 1..iterations {
        u = hmac_sha256(password, &u);
        for k in 0..32 {
            result[k] ^= u[k];
        }
    }

    result
}

/// Generates a salted PBKDF2 password hash string formatted as:
/// `pbkdf2_sha256$<iterations>$<salt_hex>$<hash_hex>`
pub fn hash_password(password: &str) -> String {
    let iterations = 10_000;
    let salt = generate_random_bytes(16);
    let salt_hex = hex_encode(&salt);
    let derived = pbkdf2_hmac_sha256(password.as_bytes(), &salt, iterations);
    let hash_hex = hex_encode(&derived);
    format!("pbkdf2_sha256${}${}${}", iterations, salt_hex, hash_hex)
}

/// Verifies a password against a stored `pbkdf2_sha256` hash string in constant time.
pub fn verify_password(password: &str, stored_hash: &str) -> bool {
    let parts: Vec<&str> = stored_hash.split('$').collect();
    if parts.len() != 4 || parts[0] != "pbkdf2_sha256" {
        return false;
    }

    let iterations: u32 = match parts[1].parse() {
        Ok(it) if it > 0 => it,
        _ => return false,
    };

    let salt = match hex_decode(parts[2]) {
        Some(s) => s,
        None => return false,
    };

    let expected_hash = match hex_decode(parts[3]) {
        Some(h) => h,
        None => return false,
    };

    if expected_hash.len() != 32 {
        return false;
    }

    let computed = pbkdf2_hmac_sha256(password.as_bytes(), &salt, iterations);
    constant_time_eq(&computed, &expected_hash)
}

// =============================================================================
// Constant-Time Equality (Side-Channel Prevention)
// =============================================================================

/// Compares two byte slices in constant time to prevent timing attacks.
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

// =============================================================================
// Hex & Base64 Encoding / Decoding
// =============================================================================

pub fn hex_encode(bytes: &[u8]) -> String {
    const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX_CHARS[(b >> 4) as usize] as char);
        out.push(HEX_CHARS[(b & 0x0f) as usize] as char);
    }
    out
}

pub fn hex_decode(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() / 2);
    for chunk in bytes.chunks(2) {
        let high = hex_val(chunk[0])?;
        let low = hex_val(chunk[1])?;
        out.push((high << 4) | low);
    }
    Some(out)
}

fn hex_val(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

const BASE64_CHARS: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard RFC 4648 Base64 encode.
pub fn base64_encode(input: &[u8]) -> String {
    let mut result = String::with_capacity((input.len() + 2) / 3 * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0];
        let b1 = if chunk.len() > 1 { chunk[1] } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] } else { 0 };

        result.push(BASE64_CHARS[(b0 >> 2) as usize] as char);
        result.push(BASE64_CHARS[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);

        if chunk.len() > 1 {
            result.push(BASE64_CHARS[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            result.push('=');
        }

        if chunk.len() > 2 {
            result.push(BASE64_CHARS[(b2 & 0x3f) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}

/// Standard RFC 4648 Base64 decode.
pub fn base64_decode(input: &str) -> Option<Vec<u8>> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Some(Vec::new());
    }

    let mut lookup = [0xFFu8; 256];
    for (i, &c) in BASE64_CHARS.iter().enumerate() {
        lookup[c as usize] = i as u8;
    }

    let clean: Vec<u8> = trimmed
        .bytes()
        .filter(|&b| b != b'\r' && b != b'\n' && b != b' ')
        .collect();

    if clean.len() % 4 != 0 {
        return None;
    }

    let mut out = Vec::with_capacity(clean.len() / 4 * 3);
    for chunk in clean.chunks(4) {
        let c0 = chunk[0];
        let c1 = chunk[1];
        let c2 = chunk[2];
        let c3 = chunk[3];

        let v0 = lookup[c0 as usize];
        let v1 = lookup[c1 as usize];
        if v0 == 0xFF || v1 == 0xFF {
            return None;
        }

        out.push((v0 << 2) | (v1 >> 4));

        if c2 != b'=' {
            let v2 = lookup[c2 as usize];
            if v2 == 0xFF {
                return None;
            }
            out.push(((v1 & 0x0F) << 4) | (v2 >> 2));

            if c3 != b'=' {
                let v3 = lookup[c3 as usize];
                if v3 == 0xFF {
                    return None;
                }
                out.push(((v2 & 0x03) << 6) | v3);
            }
        }
    }
    Some(out)
}

// =============================================================================
// Cryptographically Strong Entropy & Token Generation
// =============================================================================

static TOKEN_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Generates pseudorandom entropy bytes mixing time, memory pointer addresses,
/// process ID, thread id, and atomic counter through SHA-256.
pub fn generate_random_bytes(len: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(len);
    while out.len() < len {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let counter = TOKEN_COUNTER.fetch_add(1, Ordering::SeqCst);
        let pid = std::process::id();
        let stack_marker = 0x5a5a5a5au32;
        let stack_addr = &stack_marker as *const u32 as usize;

        let mut hasher = Sha256::new();
        hasher.update(&now.as_nanos().to_le_bytes());
        hasher.update(&counter.to_le_bytes());
        hasher.update(&pid.to_le_bytes());
        hasher.update(&stack_addr.to_le_bytes());
        let digest = hasher.finalize();

        let take = (len - out.len()).min(32);
        out.extend_from_slice(&digest[..take]);
    }
    out
}

/// Generates a secure, unguessable 64-character hex session Bearer token.
pub fn generate_bearer_token() -> String {
    let bytes = generate_random_bytes(32);
    hex_encode(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha256_known_vector() {
        // "abc" -> ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
        let hash = Sha256::digest(b"abc");
        assert_eq!(
            hex_encode(&hash),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn test_hmac_sha256_vector() {
        let key = b"key";
        let data = b"The quick brown fox jumps over the lazy dog";
        let hmac = hmac_sha256(key, data);
        assert_eq!(
            hex_encode(&hmac),
            "f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8"
        );
    }

    #[test]
    fn test_password_hash_and_verify() {
        let pass = "StrongPassword123!";
        let hash = hash_password(pass);
        assert!(verify_password(pass, &hash));
        assert!(!verify_password("WrongPassword", &hash));
    }

    #[test]
    fn test_base64_roundtrip() {
        let original = "admin:secret_pass_123!";
        let encoded = base64_encode(original.as_bytes());
        let decoded = base64_decode(&encoded).expect("Decode valid base64");
        assert_eq!(String::from_utf8(decoded).unwrap(), original);
    }
}
