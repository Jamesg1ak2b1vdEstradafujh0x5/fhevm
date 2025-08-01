//! Signature module for FHEVM SDK
//!
//! This module provides EIP-712 signature generation and ML-KEM keypair functionality.

use crate::{FhevmError, Result};
use alloy::primitives::{Address, B256, Bytes};
use alloy::signers::local::PrivateKeySigner;
use kms_lib::client::js_api::{self, ml_kem_pke_pk_to_u8vec, ml_kem_pke_sk_to_u8vec};
use serde::{Deserialize, Serialize};

// Sub-modules
pub mod eip712;

// Re-export main types
pub use self::eip712::{Eip712Config, Eip712Result, Eip712SignatureBuilder};

/// Keypair for ML-KEM operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Keypair {
    pub public_key: String,
    pub private_key: String,
}

/// Generate a new keypair for ML-KEM operations
pub fn generate_keypair() -> Result<Keypair> {
    // Generate private key using the JS API
    let private_key = js_api::ml_kem_pke_keygen();
    let public_key = js_api::ml_kem_pke_get_pk(&private_key);

    let priv_key = ml_kem_pke_sk_to_u8vec(&private_key)
        .map_err(|_| FhevmError::SignatureError("Failed to convert private key to bytes".into()))?;

    let pub_key = ml_kem_pke_pk_to_u8vec(&public_key)
        .map_err(|_| FhevmError::SignatureError("Failed to convert public key to bytes".into()))?;

    Ok(Keypair {
        public_key: format!("0x{}", hex::encode(pub_key)),
        private_key: format!("0x{}", hex::encode(priv_key)),
    })
}

/// Validate private key format
pub fn validate_private_key_format(private_key: &str) -> Result<()> {
    if private_key.is_empty() {
        return Err(FhevmError::InvalidParams(
            "Private key cannot be empty".to_string(),
        ));
    }

    // Remove 0x prefix if present
    let cleaned_key = private_key.strip_prefix("0x").unwrap_or(private_key);

    // Check length (64 hex characters = 32 bytes)
    if cleaned_key.len() != 64 {
        return Err(FhevmError::InvalidParams(
            "Invalid private key format (must be 64 hex characters)".to_string(),
        ));
    }

    // Verify it's valid hex
    if !cleaned_key.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(FhevmError::InvalidParams(
            "Invalid private key format (contains non-hex characters)".to_string(),
        ));
    }

    Ok(())
}

/// Sign an EIP-712 hash with a private key
///
/// Signs the provided hash using ECDSA with the given private key
pub(crate) fn sign_eip712_hash(hash: B256, private_key: &str) -> Result<Bytes> {
    use alloy::signers::{Signer, local::PrivateKeySigner};
    use std::str::FromStr;

    // Parse the private key (remove 0x prefix if present)
    let private_key_str = private_key.strip_prefix("0x").unwrap_or(private_key);

    // Create the signer
    let signer = PrivateKeySigner::from_str(private_key_str)
        .map_err(|e| FhevmError::SignatureError(format!("Invalid private key: {e}")))?;

    // Try to use existing runtime, fallback to blocking if needed
    let signature = if let Ok(handle) = tokio::runtime::Handle::try_current() {
        // We're already in a tokio runtime
        handle.block_on(async { signer.sign_hash(&hash).await })
    } else {
        // No runtime exists, create a minimal one
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| FhevmError::SignatureError(format!("Failed to create runtime: {e}")))?;

        rt.block_on(async { signer.sign_hash(&hash).await })
    };

    let signature =
        signature.map_err(|e| FhevmError::SignatureError(format!("Failed to sign: {e}")))?;

    Ok(Bytes::from(signature.as_bytes().to_vec()))
}

/// Derive Ethereum address from private key
pub fn derive_address_from_private_key(private_key: &str) -> Result<Address> {
    use std::str::FromStr;

    let private_key_str = private_key.strip_prefix("0x").unwrap_or(private_key);

    let signer = PrivateKeySigner::from_str(private_key_str)
        .map_err(|e| FhevmError::SignatureError(format!("Invalid private key: {e}")))?;

    Ok(signer.address())
}
