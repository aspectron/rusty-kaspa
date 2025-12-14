//!
//! Key hashing utilities for enhanced privacy in UTXO management.
//!
//! This module provides functionality to create addresses from hashed public keys,
//! improving privacy by not exposing the actual public key until the UTXO is spent.

use crate::imports::*;
use kaspa_addresses::{Address, Prefix, Version};
use kaspa_hashes::{Hash, Hasher, TransactionHash};
use secp256k1::PublicKey;

/// Creates a hash of a public key for use in UTXO addresses
pub fn hash_public_key(public_key: &PublicKey) -> Hash {
    TransactionHash::hash(&public_key.serialize())
}

/// Creates an address from a hashed public key
pub fn create_hashed_address(public_key: &PublicKey, prefix: Prefix) -> Result<Address> {
    let key_hash = hash_public_key(public_key);
    let address = Address::new(prefix, Version::PubKeyHash, &key_hash.as_bytes());
    Ok(address)
}

/// Creates an address from a hashed public key for Schnorr signatures
pub fn create_hashed_schnorr_address(public_key: &secp256k1::XOnlyPublicKey, prefix: Prefix) -> Result<Address> {
    let key_hash = TransactionHash::hash(&public_key.serialize());
    let address = Address::new(prefix, Version::PubKeyHash, &key_hash.as_bytes());
    Ok(address)
}

/// Derives the next change address using hashed public key
pub fn derive_next_change_address_hashed(public_key: &PublicKey, change_index: u32, prefix: Prefix) -> Result<Address> {
    // We'll use a simple derivation by hashing the public key with the index
    // In a real implementation, this would use proper HD derivation
    let mut data = public_key.serialize().to_vec();
    data.extend_from_slice(&change_index.to_le_bytes());
    let key_hash = TransactionHash::hash(&data);
    let address = Address::new(prefix, Version::PubKeyHash, &key_hash.as_bytes());
    Ok(address)
}

#[cfg(test)]
mod tests {
    use super::*;
    use secp256k1::Secp256k1;

    #[test]
    fn test_hash_public_key() {
        let secp = Secp256k1::new();
        let (_, public_key) = secp.generate_keypair(&mut secp256k1::rand::thread_rng());

        let hash1 = hash_public_key(&public_key);
        let hash2 = hash_public_key(&public_key);

        // Same public key should produce same hash
        assert_eq!(hash1, hash2);

        // Hash should be different from the original public key
        assert_ne!(hash1.as_bytes(), &public_key.serialize()[..]);
    }
    #[test]
    fn test_create_hashed_address_compare() -> Result<()> {
        println!("\n=== Key Hashing for Enhanced Privacy Example ===");

        let secp = Secp256k1::new();
        let (_secret_key, public_key) = secp.generate_keypair(&mut secp256k1::rand::thread_rng());

        // Create regular address (exposes public key)
        let regular_address = Address::new(Prefix::Testnet, Version::PubKeyECDSA, &public_key.serialize());
        println!("Regular Address (exposes public key): {}", regular_address);

        // Create hashed address (hides public key until spent)
        let hashed_address = create_hashed_address(&public_key, Prefix::Testnet)?;
        println!("Hashed Address (privacy enhanced): {}", hashed_address);

        // Demonstrate that the hashed address doesn't reveal the public key
        println!("Public key: {}", faster_hex::hex_string(&public_key.serialize()));
        println!("Hashed address payload: {}", faster_hex::hex_string(hashed_address.payload.as_slice()));

        // Show that different public keys produce different hashed addresses
        let (_, another_public_key) = secp.generate_keypair(&mut secp256k1::rand::thread_rng());
        let another_hashed_address = create_hashed_address(&another_public_key, Prefix::Testnet)?;
        println!("Another hashed address: {}", another_hashed_address);

        assert_eq!(another_hashed_address.version, Version::PubKeyHash);
        assert_eq!(another_hashed_address.prefix, Prefix::Testnet);
        assert_eq!(another_hashed_address.payload.len(), 32);

        assert_eq!(hashed_address.version, Version::PubKeyHash);
        assert_eq!(hashed_address.prefix, Prefix::Testnet);
        assert_eq!(hashed_address.payload.len(), 32);

        assert_ne!(hashed_address, another_hashed_address);
        println!("✓ Different public keys produce different hashed addresses");

        Ok(())
    }

    #[test]
    fn test_derive_next_change_address() {
        let secp = Secp256k1::new();
        let (_, public_key) = secp.generate_keypair(&mut secp256k1::rand::thread_rng());

        let address1 = derive_next_change_address_hashed(&public_key, 0, Prefix::Mainnet).unwrap();
        let address2 = derive_next_change_address_hashed(&public_key, 1, Prefix::Mainnet).unwrap();

        // Different indices should produce different addresses
        assert_ne!(address1, address2);

        // Both should be PubKeyHash addresses
        assert_eq!(address1.version, Version::PubKeyHash);
        assert_eq!(address2.version, Version::PubKeyHash);
    }
}
