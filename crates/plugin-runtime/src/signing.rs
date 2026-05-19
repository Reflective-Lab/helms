// Copyright 2024-2026 Reflective Labs
// SPDX-License-Identifier: MIT

//! WASM module signing and integrity verification.
//!
//! Provides ed25519-based digital signatures for WASM modules to prevent
//! supply chain attacks. Every module must be signed before deployment,
//! and Helm verifies the signature before loading.
//!
//! # Signing Flow
//!
//! ```text
//! build → sign(wasm_bytes, secret_key) → (wasm_bytes, ModuleSignature)
//! load  → verify(wasm_bytes, signature, public_key) → Ok(()) | Err
//! ```
//!
//! # Feature Gate
//!
//!
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::contract::WasmError;

/// A detached ed25519 signature over WASM module bytes.
///
/// The signature covers `SHA-256(wasm_bytes)`, not the raw bytes directly.
/// This allows the registry to store and transmit signatures without
/// needing the full module bytes for comparison.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleSignature {
    /// The ed25519 signature bytes, hex-encoded.
    pub signature_hex: String,
    /// The public key that produced this signature, hex-encoded.
    pub public_key_hex: String,
    /// The SHA-256 digest that was signed, hex-encoded.
    pub digest_hex: String,
}

/// Policy controlling whether signature verification is enforced.
///
/// During development and testing, modules may be loaded without
/// signatures. In staging and production, signatures are mandatory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignaturePolicy {
    /// All modules must have a valid signature. Unsigned or tampered
    /// modules are rejected.
    Enforce,
    /// Signatures are checked if present, but unsigned modules are
    /// allowed. Use only in development.
    WarnOnly,
    /// No signature checking. Use only in unit tests.
    Disabled,
}

impl Default for SignaturePolicy {
    fn default() -> Self {
        Self::Enforce
    }
}

/// A set of trusted public keys for signature verification.
///
/// The registry maintains a list of keys authorized to sign modules.
/// A module's signature must be produced by one of these keys to be
/// accepted.
#[derive(Debug, Clone)]
pub struct TrustedKeySet {
    keys: Vec<VerifyingKey>,
}

impl TrustedKeySet {
    /// Create an empty key set (rejects all signatures).
    pub fn empty() -> Self {
        Self { keys: Vec::new() }
    }

    /// Create a key set from hex-encoded ed25519 public keys.
    ///
    /// Invalid keys are silently skipped. Returns the count of
    /// successfully parsed keys.
    pub fn from_hex_keys(hex_keys: &[&str]) -> (Self, usize) {
        let mut keys = Vec::with_capacity(hex_keys.len());
        for hex_key in hex_keys {
            if let Some(key) = parse_verifying_key(hex_key) {
                keys.push(key);
            }
        }
        let count = keys.len();
        (Self { keys }, count)
    }

    /// Add a verifying key to the trusted set.
    pub fn add_key(&mut self, key: VerifyingKey) {
        self.keys.push(key);
    }

    /// Returns true if this key set contains at least one key.
    pub fn has_keys(&self) -> bool {
        !self.keys.is_empty()
    }

    /// Number of trusted keys.
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    /// Returns true if there are no trusted keys.
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }
}

/// Sign WASM module bytes with an ed25519 signing key.
///
/// Returns a detached `ModuleSignature` that can be stored alongside
/// the module in the registry.
pub fn sign_module(wasm_bytes: &[u8], signing_key: &SigningKey) -> ModuleSignature {
    let digest = Sha256::digest(wasm_bytes);
    let signature = signing_key.sign(&digest);
    let verifying_key = signing_key.verifying_key();

    ModuleSignature {
        signature_hex: hex::encode(signature.to_bytes()),
        public_key_hex: hex::encode(verifying_key.to_bytes()),
        digest_hex: hex::encode(digest),
    }
}

/// Verify a module signature against its bytes and a set of trusted keys.
///
/// Checks that:
/// 1. The signature's digest matches `SHA-256(wasm_bytes)`
/// 2. The signing public key is in the trusted key set
/// 3. The ed25519 signature is valid
///
/// # Errors
///
/// - `WasmError::SignatureInvalid` if any check fails
pub fn verify_module(
    wasm_bytes: &[u8],
    signature: &ModuleSignature,
    trusted_keys: &TrustedKeySet,
) -> Result<(), WasmError> {
    // 1. Recompute digest and compare
    let actual_digest = Sha256::digest(wasm_bytes);
    let actual_digest_hex = hex::encode(actual_digest);

    if actual_digest_hex != signature.digest_hex {
        return Err(WasmError::SignatureInvalid(
            "module bytes do not match signed digest (tampered content)".to_string(),
        ));
    }

    // 2. Parse the signature's public key
    let signer_key = parse_verifying_key(&signature.public_key_hex).ok_or_else(|| {
        WasmError::SignatureInvalid("invalid public key in signature".to_string())
    })?;

    // 3. Check that the signer is trusted
    let is_trusted = trusted_keys
        .keys
        .iter()
        .any(|k| k.as_bytes() == signer_key.as_bytes());
    if !is_trusted {
        return Err(WasmError::SignatureInvalid(
            "signing key is not in the trusted key set".to_string(),
        ));
    }

    // 4. Parse and verify the ed25519 signature
    let sig_bytes = hex::decode(&signature.signature_hex)
        .map_err(|_| WasmError::SignatureInvalid("signature hex is malformed".to_string()))?;
    let sig = Signature::from_slice(&sig_bytes)
        .map_err(|_| WasmError::SignatureInvalid("invalid ed25519 signature bytes".to_string()))?;

    signer_key.verify(&actual_digest, &sig).map_err(|_| {
        WasmError::SignatureInvalid("ed25519 signature verification failed".to_string())
    })?;

    Ok(())
}

/// Verify a module signature according to the given policy.
///
/// This is the main entry point for the module loading path.
///
/// # Errors
///
/// - `WasmError::SignatureMissing` if policy is `Enforce` and no signature provided
/// - `WasmError::SignatureInvalid` if signature verification fails
pub fn verify_module_with_policy(
    wasm_bytes: &[u8],
    signature: Option<&ModuleSignature>,
    trusted_keys: &TrustedKeySet,
    policy: &SignaturePolicy,
) -> Result<(), WasmError> {
    match policy {
        SignaturePolicy::Disabled => Ok(()),
        SignaturePolicy::WarnOnly => {
            if let Some(sig) = signature {
                if let Err(e) = verify_module(wasm_bytes, sig, trusted_keys) {
                    // In warn-only mode, log but don't reject.
                    // The caller should capture this for audit.
                    tracing::warn!(error = %e, "module signature verification failed (warn-only mode)");
                }
            }
            Ok(())
        }
        SignaturePolicy::Enforce => {
            let sig = signature.ok_or_else(|| {
                WasmError::SignatureMissing(
                    "module signature is required (policy: enforce)".to_string(),
                )
            })?;
            verify_module(wasm_bytes, sig, trusted_keys)
        }
    }
}

/// Parse a hex-encoded ed25519 verifying (public) key.
fn parse_verifying_key(hex_key: &str) -> Option<VerifyingKey> {
    let bytes = hex::decode(hex_key).ok()?;
    let arr: [u8; 32] = bytes.try_into().ok()?;
    VerifyingKey::from_bytes(&arr).ok()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;

    fn keypair_from_seed(seed: u8) -> (SigningKey, VerifyingKey) {
        let signing_key = SigningKey::from_bytes(&[seed; 32]);
        let verifying_key = signing_key.verifying_key();
        (signing_key, verifying_key)
    }

    fn generate_keypair() -> (SigningKey, VerifyingKey) {
        keypair_from_seed(7)
    }

    fn sample_wasm_bytes() -> Vec<u8> {
        // Minimal valid-ish bytes for testing (not real WASM, just for signing tests)
        b"(module (memory 1))".to_vec()
    }

    // =========================================================================
    // Happy path
    // =========================================================================

    #[test]
    fn sign_and_verify_roundtrip() {
        let (signing_key, verifying_key) = generate_keypair();
        let wasm = sample_wasm_bytes();

        let sig = sign_module(&wasm, &signing_key);

        let mut trusted = TrustedKeySet::empty();
        trusted.add_key(verifying_key);

        assert!(verify_module(&wasm, &sig, &trusted).is_ok());
    }

    #[test]
    fn verify_with_policy_enforce_valid_signature() {
        let (signing_key, verifying_key) = generate_keypair();
        let wasm = sample_wasm_bytes();
        let sig = sign_module(&wasm, &signing_key);

        let mut trusted = TrustedKeySet::empty();
        trusted.add_key(verifying_key);

        let result =
            verify_module_with_policy(&wasm, Some(&sig), &trusted, &SignaturePolicy::Enforce);
        assert!(result.is_ok());
    }

    #[test]
    fn verify_with_policy_disabled_accepts_anything() {
        let wasm = sample_wasm_bytes();
        let trusted = TrustedKeySet::empty();

        // No signature, disabled policy → ok
        let result = verify_module_with_policy(&wasm, None, &trusted, &SignaturePolicy::Disabled);
        assert!(result.is_ok());
    }

    #[test]
    fn verify_with_policy_warn_only_accepts_unsigned() {
        let wasm = sample_wasm_bytes();
        let trusted = TrustedKeySet::empty();

        let result = verify_module_with_policy(&wasm, None, &trusted, &SignaturePolicy::WarnOnly);
        assert!(result.is_ok());
    }

    // =========================================================================
    // Tampered content (supply chain attack simulation)
    // =========================================================================

    #[test]
    fn tampered_bytes_are_rejected() {
        let (signing_key, verifying_key) = generate_keypair();
        let wasm = sample_wasm_bytes();
        let sig = sign_module(&wasm, &signing_key);

        let mut trusted = TrustedKeySet::empty();
        trusted.add_key(verifying_key);

        // Tamper with the module bytes
        let mut tampered = wasm.clone();
        tampered.push(0xFF);

        let result = verify_module(&tampered, &sig, &trusted);
        assert!(result.is_err());
        assert!(matches!(result, Err(WasmError::SignatureInvalid(_))));
    }

    #[test]
    fn tampered_bytes_rejected_under_enforce_policy() {
        let (signing_key, verifying_key) = generate_keypair();
        let wasm = sample_wasm_bytes();
        let sig = sign_module(&wasm, &signing_key);

        let mut trusted = TrustedKeySet::empty();
        trusted.add_key(verifying_key);

        let mut tampered = wasm;
        tampered[0] = 0x00;

        let result =
            verify_module_with_policy(&tampered, Some(&sig), &trusted, &SignaturePolicy::Enforce);
        assert!(result.is_err());
        assert!(matches!(result, Err(WasmError::SignatureInvalid(_))));
    }

    // =========================================================================
    // Missing signature
    // =========================================================================

    #[test]
    fn enforce_policy_rejects_missing_signature() {
        let wasm = sample_wasm_bytes();
        let trusted = TrustedKeySet::empty();

        let result = verify_module_with_policy(&wasm, None, &trusted, &SignaturePolicy::Enforce);
        assert!(result.is_err());
        assert!(matches!(result, Err(WasmError::SignatureMissing(_))));
    }

    // =========================================================================
    // Untrusted signer
    // =========================================================================

    #[test]
    fn untrusted_signer_is_rejected() {
        let (signing_key, _) = generate_keypair();
        let (_, other_verifying_key) = keypair_from_seed(8);
        let wasm = sample_wasm_bytes();
        let sig = sign_module(&wasm, &signing_key);

        // Trust a different key than the one that signed
        let mut trusted = TrustedKeySet::empty();
        trusted.add_key(other_verifying_key);

        let result = verify_module(&wasm, &sig, &trusted);
        assert!(result.is_err());
        assert!(matches!(result, Err(WasmError::SignatureInvalid(_))));
    }

    // =========================================================================
    // Forged signature
    // =========================================================================

    #[test]
    fn forged_signature_is_rejected() {
        let (signing_key, verifying_key) = generate_keypair();
        let wasm = sample_wasm_bytes();

        // Sign different content
        let different_bytes = b"different module content";
        let wrong_sig = sign_module(different_bytes, &signing_key);

        let mut trusted = TrustedKeySet::empty();
        trusted.add_key(verifying_key);

        let result = verify_module(&wasm, &wrong_sig, &trusted);
        assert!(result.is_err());
    }

    // =========================================================================
    // Key set management
    // =========================================================================

    #[test]
    fn trusted_key_set_from_hex() {
        let (_, verifying_key) = generate_keypair();
        let hex_key = hex::encode(verifying_key.to_bytes());

        let (key_set, count) = TrustedKeySet::from_hex_keys(&[&hex_key]);
        assert_eq!(count, 1);
        assert!(key_set.has_keys());
    }

    #[test]
    fn invalid_hex_keys_are_skipped() {
        let (key_set, count) = TrustedKeySet::from_hex_keys(&["not-valid-hex", "0000"]);
        assert_eq!(count, 0);
        assert!(key_set.is_empty());
    }

    #[test]
    fn multiple_trusted_keys() {
        let (signing_key_1, verifying_key_1) = generate_keypair();
        let (_, verifying_key_2) = generate_keypair();
        let wasm = sample_wasm_bytes();
        let sig = sign_module(&wasm, &signing_key_1);

        let mut trusted = TrustedKeySet::empty();
        trusted.add_key(verifying_key_1);
        trusted.add_key(verifying_key_2);

        // Signed by key 1, both keys trusted → ok
        assert!(verify_module(&wasm, &sig, &trusted).is_ok());
    }

    // =========================================================================
    // Signature struct serialization
    // =========================================================================

    #[test]
    fn module_signature_roundtrip_json() {
        let (signing_key, _) = generate_keypair();
        let wasm = sample_wasm_bytes();
        let sig = sign_module(&wasm, &signing_key);

        let json = serde_json::to_string(&sig).expect("serialize");
        let deserialized: ModuleSignature = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(sig.signature_hex, deserialized.signature_hex);
        assert_eq!(sig.public_key_hex, deserialized.public_key_hex);
        assert_eq!(sig.digest_hex, deserialized.digest_hex);
    }
}
