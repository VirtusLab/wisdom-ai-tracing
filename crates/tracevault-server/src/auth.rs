use argon2::password_hash::rand_core::{OsRng, RngCore};
use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use sha2::{Digest, Sha256};

/// Hash a password with Argon2id. Returns PHC-format string.
pub fn hash_password(password: &str) -> Result<String, argon2::password_hash::Error> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    Ok(argon2
        .hash_password(password.as_bytes(), &salt)?
        .to_string())
}

/// Verify a password against a PHC-format hash.
pub fn verify_password(password: &str, hash: &str) -> bool {
    let parsed = match PasswordHash::new(hash) {
        Ok(h) => h,
        Err(_) => return false,
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

fn random_bytes_32() -> [u8; 32] {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    bytes
}

/// Generate a random session token. Returns (raw_token, sha256_hash).
/// Raw token is prefixed with `tvs_` and hex-encoded.
pub fn generate_session_token() -> (String, String) {
    let bytes = random_bytes_32();
    let raw = format!("tvs_{}", hex::encode(bytes));
    let hash = sha256_hex(&raw);
    (raw, hash)
}

/// Generate a random API key. Returns (raw_key, sha256_hash).
/// Raw key is prefixed with `tvk_` and hex-encoded.
pub fn generate_api_key() -> (String, String) {
    let bytes = random_bytes_32();
    let raw = format!("tvk_{}", hex::encode(bytes));
    let hash = sha256_hex(&raw);
    (raw, hash)
}

/// Generate a random device auth token (hex-encoded, no prefix).
pub fn generate_device_token() -> String {
    let bytes = random_bytes_32();
    hex::encode(bytes)
}

/// Generate a random invite token. Returns (raw_hex, sha256_hash).
pub fn generate_invite_token() -> (String, String) {
    let bytes = random_bytes_32();
    let raw = hex::encode(bytes);
    let hash = sha256_hex(&raw);
    (raw, hash)
}

/// SHA-256 hash a string, return hex-encoded.
pub fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
}

/// Single-statement sliding-session auth check, shared by both request
/// extractors (`AuthUser`, `OrgAuth`) so the behavior is defined once.
///
/// Binds `$1` = `token_hash` and returns the matching `user_id`, or no row
/// when the token is missing or expired (the `expires_at > NOW()` filter
/// rejects expired sessions — they are never revived).
///
/// Sliding window: as long as a user authenticates at least once per ~30
/// days the token never expires; genuinely-dormant tokens still hit the
/// cliff and must re-login.
///
/// Why a CTE instead of a plain `UPDATE ... RETURNING`: an unconditional
/// UPDATE would take a row lock and write a new tuple on *every*
/// authenticated request. TraceVault streams an event on every agent hook,
/// so the same token authenticates many times in quick succession (often
/// concurrently) — an always-write check serializes those requests on one
/// row lock and amplifies WAL/IO. Here the `session` CTE always reads the
/// `user_id` for the auth decision, while the `slide` CTE only writes when
/// the expiry has drifted more than a day from the 30-day target. The
/// common "already-fresh token" path therefore performs no write and takes
/// no row lock. A data-modifying CTE always runs to completion even though
/// nothing selects from it, so the slide still happens when due.
pub const SLIDING_SESSION_AUTH_SQL: &str = "\
WITH session AS (
    SELECT user_id FROM auth_sessions
    WHERE token_hash = $1 AND expires_at > NOW()
),
slide AS (
    UPDATE auth_sessions
    SET expires_at = NOW() + INTERVAL '30 days'
    WHERE token_hash = $1
      AND expires_at > NOW()
      AND expires_at < NOW() + INTERVAL '29 days'
)
SELECT user_id FROM session";

#[cfg(test)]
mod tests {
    use super::*;

    fn test_password() -> String {
        format!("test-{}-password", 123)
    }

    #[test]
    fn hash_and_verify_roundtrip() {
        let pw = test_password();
        let hash = hash_password(&pw).unwrap();
        assert!(verify_password(&pw, &hash));
    }

    #[test]
    fn verify_wrong_password() {
        let hash = hash_password(&test_password()).unwrap();
        assert!(!verify_password("wrong", &hash));
    }

    #[test]
    fn verify_invalid_hash() {
        assert!(!verify_password("anything", "not-a-valid-hash"));
    }

    #[test]
    fn session_token_format() {
        let (raw, hash) = generate_session_token();
        assert!(raw.starts_with("tvs_"));
        assert_eq!(hash.len(), 64);
        assert_eq!(sha256_hex(&raw), hash);
    }

    #[test]
    fn api_key_format() {
        let (raw, hash) = generate_api_key();
        assert!(raw.starts_with("tvk_"));
        assert_eq!(hash.len(), 64);
        assert_eq!(sha256_hex(&raw), hash);
    }

    #[test]
    fn device_token_length() {
        let token = generate_device_token();
        assert_eq!(token.len(), 64);
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn sha256_hex_deterministic() {
        let a = sha256_hex("hello");
        let b = sha256_hex("hello");
        assert_eq!(a, b);
        assert_ne!(sha256_hex("hello"), sha256_hex("world"));
    }

    #[test]
    fn invite_token_format() {
        let (raw, hash) = generate_invite_token();
        assert_eq!(raw.len(), 64);
        assert!(raw.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(hash.len(), 64);
        assert_eq!(sha256_hex(&raw), hash);
    }

    #[test]
    fn generated_tokens_are_unique() {
        // Pins that the OsRng-backed `random_bytes_32` doesn't collide across
        // calls — collisions here would let an attacker predict session
        // tokens. Trivially improbable but the test catches a broken RNG
        // wiring (e.g. a fixed seed leaking in via an upgrade).
        let (a, _) = generate_session_token();
        let (b, _) = generate_session_token();
        assert_ne!(a, b);

        let (c, _) = generate_api_key();
        let (d, _) = generate_api_key();
        assert_ne!(c, d);

        assert_ne!(generate_device_token(), generate_device_token());
    }

    #[test]
    fn argon2_salts_are_unique() {
        // Same password hashed twice must produce different PHC strings,
        // because `SaltString::generate(&mut OsRng)` must produce fresh salt.
        let pw = test_password();
        let h1 = hash_password(&pw).unwrap();
        let h2 = hash_password(&pw).unwrap();
        assert_ne!(h1, h2);
        assert!(verify_password(&pw, &h1));
        assert!(verify_password(&pw, &h2));
    }
}
