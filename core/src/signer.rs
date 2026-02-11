/// Signing abstraction that decouples transaction signing from a concrete key type.
///
use anyhow::Result;
use iota_sdk::crypto::ed25519::Ed25519PrivateKey;
use iota_sdk::crypto::IotaSigner;
use iota_sdk::types::{Address, Transaction, UserSignature};

pub trait Signer: Send + Sync {
    /// Sign a fully-built transaction and return the user signature.
    fn sign_transaction(&self, tx: &Transaction) -> Result<UserSignature>;

    /// The on-chain address controlled by this signer.
    fn address(&self) -> &Address;
}

/// Software signer backed by an in-memory Ed25519 private key.
pub struct SoftwareSigner {
    private_key: Ed25519PrivateKey,
    address: Address,
}

impl SoftwareSigner {
    pub fn new(private_key: Ed25519PrivateKey) -> Self {
        let address = private_key.public_key().derive_address();
        Self {
            private_key,
            address,
        }
    }
}

impl Signer for SoftwareSigner {
    fn sign_transaction(&self, tx: &Transaction) -> Result<UserSignature> {
        self.private_key
            .sign_transaction(tx)
            .map_err(|e| anyhow::anyhow!("Failed to sign transaction: {e}"))
    }

    fn address(&self) -> &Address {
        &self.address
    }
}
