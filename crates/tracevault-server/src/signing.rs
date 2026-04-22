use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use sha2::{Digest, Sha256};

#[derive(Clone)]
pub struct SigningService {
    signing_key: SigningKey,
    verifying_key: VerifyingKey,
}

impl SigningService {
    /// Create from a base64-encoded 32-byte seed, or generate a new key.
    pub fn new(seed_b64: Option<&str>) -> Self {
        let signing_key = if let Some(seed) = seed_b64 {
            let bytes = BASE64
                .decode(seed)
                .expect("Invalid base64 signing key seed");
            let seed_bytes: [u8; 32] = bytes.try_into().expect("Signing key seed must be 32 bytes");
            SigningKey::from_bytes(&seed_bytes)
        } else {
            tracing::warn!("No TRACEVAULT_SIGNING_KEY set — using ephemeral key. Signatures will not survive restart.");
            SigningKey::generate(&mut rand::thread_rng())
        };
        let verifying_key = signing_key.verifying_key();
        Self {
            signing_key,
            verifying_key,
        }
    }

    /// Sign a record hash and return the signature as base64.
    pub fn sign(&self, record_hash: &str) -> String {
        let sig = self.signing_key.sign(record_hash.as_bytes());
        BASE64.encode(sig.to_bytes())
    }

    /// Compute chain hash: SHA-256(prev_chain_hash || record_hash).
    pub fn chain_hash(&self, prev_chain_hash: Option<&str>, record_hash: &str) -> String {
        let mut hasher = Sha256::new();
        if let Some(prev) = prev_chain_hash {
            hasher.update(prev.as_bytes());
        }
        hasher.update(record_hash.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Verify a signature against a record hash.
    pub fn verify(&self, record_hash: &str, signature_b64: &str) -> bool {
        let sig_bytes = match BASE64.decode(signature_b64) {
            Ok(b) => b,
            Err(_) => return false,
        };
        let sig = match Signature::from_slice(&sig_bytes) {
            Ok(s) => s,
            Err(_) => return false,
        };
        self.verifying_key
            .verify(record_hash.as_bytes(), &sig)
            .is_ok()
    }

    /// Get the public key as base64 for distribution.
    pub fn public_key_b64(&self) -> String {
        BASE64.encode(self.verifying_key.as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_seed() -> String {
        BASE64.encode([42u8; 32])
    }

    #[test]
    fn new_ephemeral_creates_service() {
        let svc = SigningService::new(None);
        assert!(!svc.public_key_b64().is_empty());
    }

    #[test]
    fn new_with_seed_deterministic() {
        let seed = test_seed();
        let svc1 = SigningService::new(Some(&seed));
        let svc2 = SigningService::new(Some(&seed));
        assert_eq!(svc1.public_key_b64(), svc2.public_key_b64());
    }

    #[test]
    fn chain_hash_no_prev() {
        let svc = SigningService::new(None);
        let hash = svc.chain_hash(None, "record123");
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn chain_hash_with_prev_differs() {
        let svc = SigningService::new(None);
        let h1 = svc.chain_hash(None, "record123");
        let h2 = svc.chain_hash(Some("prevhash"), "record123");
        assert_ne!(h1, h2);
    }

    #[test]
    fn chain_hash_deterministic() {
        let svc = SigningService::new(None);
        let h1 = svc.chain_hash(Some("prev"), "record");
        let h2 = svc.chain_hash(Some("prev"), "record");
        assert_eq!(h1, h2);
    }

    #[test]
    fn verify_valid_signature() {
        let seed = test_seed();
        let svc = SigningService::new(Some(&seed));
        let key_bytes = BASE64.decode(&seed).unwrap();
        let signing_key = SigningKey::from_bytes(key_bytes[..32].try_into().unwrap());
        let sig = signing_key.sign(b"myhash");
        let sig_b64 = BASE64.encode(sig.to_bytes());
        assert!(svc.verify("myhash", &sig_b64));
    }

    #[test]
    fn verify_tampered_message() {
        let seed = test_seed();
        let svc = SigningService::new(Some(&seed));
        let key_bytes = BASE64.decode(&seed).unwrap();
        let signing_key = SigningKey::from_bytes(key_bytes[..32].try_into().unwrap());
        let sig = signing_key.sign(b"original");
        let sig_b64 = BASE64.encode(sig.to_bytes());
        assert!(!svc.verify("tampered", &sig_b64));
    }

    #[test]
    fn verify_invalid_base64() {
        let svc = SigningService::new(None);
        assert!(!svc.verify("hash", "not-valid-base64!!!"));
    }

    #[test]
    fn sign_and_verify_roundtrip() {
        let seed = test_seed();
        let svc = SigningService::new(Some(&seed));
        let sig = svc.sign("test_hash");
        assert!(svc.verify("test_hash", &sig));
        assert!(!svc.verify("wrong_hash", &sig));
    }

    #[test]
    fn ephemeral_keys_are_distinct() {
        // Two ephemeral services must produce different keypairs — pins that
        // the RNG-backed `SigningKey::generate` path isn't returning a
        // constant or reusing state across calls.
        let a = SigningService::new(None);
        let b = SigningService::new(None);
        assert_ne!(a.public_key_b64(), b.public_key_b64());
    }

    #[test]
    fn ephemeral_sign_verify_roundtrip() {
        // Cover the RNG path end-to-end: generate, sign, verify with the
        // same service (no seed involved).
        let svc = SigningService::new(None);
        let sig = svc.sign("payload");
        assert!(svc.verify("payload", &sig));
    }
}
