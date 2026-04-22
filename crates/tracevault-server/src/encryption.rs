use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    AeadCore, Aes256Gcm,
};
use base64::{engine::general_purpose::STANDARD as B64, Engine};

/// Encrypt plaintext using AES-256-GCM.
/// Returns `(ciphertext_base64, nonce_base64)`.
pub fn encrypt(plaintext: &str, key_b64: &str) -> Result<(String, String), String> {
    let key_bytes = B64
        .decode(key_b64)
        .map_err(|e| format!("invalid encryption key base64: {e}"))?;
    if key_bytes.len() != 32 {
        return Err(format!(
            "encryption key must be 32 bytes, got {}",
            key_bytes.len()
        ));
    }

    let cipher =
        Aes256Gcm::new_from_slice(&key_bytes).map_err(|e| format!("cipher init error: {e}"))?;
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(&nonce, plaintext.as_bytes())
        .map_err(|e| format!("encryption error: {e}"))?;

    Ok((B64.encode(ciphertext), B64.encode(nonce)))
}

/// Decrypt ciphertext using AES-256-GCM.
pub fn decrypt(ciphertext_b64: &str, nonce_b64: &str, key_b64: &str) -> Result<String, String> {
    let key_bytes = B64
        .decode(key_b64)
        .map_err(|e| format!("invalid encryption key base64: {e}"))?;
    if key_bytes.len() != 32 {
        return Err(format!(
            "encryption key must be 32 bytes, got {}",
            key_bytes.len()
        ));
    }

    let cipher =
        Aes256Gcm::new_from_slice(&key_bytes).map_err(|e| format!("cipher init error: {e}"))?;

    let nonce_bytes = B64
        .decode(nonce_b64)
        .map_err(|e| format!("invalid nonce base64: {e}"))?;
    let nonce = aes_gcm::Nonce::from_slice(&nonce_bytes);

    let ciphertext = B64
        .decode(ciphertext_b64)
        .map_err(|e| format!("invalid ciphertext base64: {e}"))?;

    let plaintext = cipher
        .decrypt(nonce, ciphertext.as_ref())
        .map_err(|e| format!("decryption error: {e}"))?;

    String::from_utf8(plaintext).map_err(|e| format!("invalid UTF-8 in decrypted data: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let key = B64.encode([0xABu8; 32]);
        let plaintext =
            "-----BEGIN OPENSSH PRIVATE KEY-----\ntest\n-----END OPENSSH PRIVATE KEY-----";

        let (ct, nonce) = encrypt(plaintext, &key).unwrap();
        let decrypted = decrypt(&ct, &nonce, &key).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn key_too_short() {
        let short_key = B64.encode(b"short");
        assert!(encrypt("hello", &short_key).is_err());
    }

    #[test]
    fn key_too_long() {
        let long_key = B64.encode([0u8; 64]);
        assert!(encrypt("hello", &long_key).is_err());
    }

    #[test]
    fn invalid_base64_key() {
        assert!(encrypt("hello", "not-valid-base64!!!").is_err());
    }

    #[test]
    fn decrypt_wrong_key() {
        let key1 = B64.encode([1u8; 32]);
        let key2 = B64.encode([2u8; 32]);
        let (ct, nonce) = encrypt("secret", &key1).unwrap();
        assert!(decrypt(&ct, &nonce, &key2).is_err());
    }

    #[test]
    fn decrypt_corrupted_ciphertext() {
        let key = B64.encode([3u8; 32]);
        let (_, nonce) = encrypt("secret", &key).unwrap();
        let bad_ct = B64.encode(b"corrupted");
        assert!(decrypt(&bad_ct, &nonce, &key).is_err());
    }

    #[test]
    fn decrypt_wrong_nonce() {
        let key = B64.encode([4u8; 32]);
        let (ct, _) = encrypt("secret", &key).unwrap();
        let bad_nonce = B64.encode([0u8; 12]);
        assert!(decrypt(&ct, &bad_nonce, &key).is_err());
    }

    #[test]
    fn nonce_is_unique_across_encrypts() {
        // AES-GCM nonce reuse under the same key catastrophically breaks
        // confidentiality and authenticity. This test pins that `OsRng`
        // produces a fresh nonce per encrypt call.
        let key = B64.encode([7u8; 32]);
        let (ct1, nonce1) = encrypt("same plaintext", &key).unwrap();
        let (ct2, nonce2) = encrypt("same plaintext", &key).unwrap();
        assert_ne!(nonce1, nonce2, "nonce must not repeat");
        assert_ne!(ct1, ct2, "ciphertext of equal plaintexts must differ");
    }
}
