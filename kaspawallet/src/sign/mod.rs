//! Wire-to-consensus lift, signing flows, and final-signature-script
//! assembly.
//!
//! - `wire` lifts a proto-wire `TransactionMessage` into the
//!   consensus-core `Transaction` type and builds the matching
//!   `UtxoEntry` list. Used by `parse` (consensus tx ID rendering),
//!   `mass` (post-extract mass estimation), and both signing flows.
//! - `derive` walks the BIP-32 chain to the leaf signing key and
//!   matches it against the input's stored cosigner xpub.
//! - `schnorr` signs every input of a PST in Schnorr (BIP-340)
//!   mode against a single mnemonic.
//! - `ecdsa` signs every input of a PST in ECDSA (RFC 6979) mode
//!   against a single mnemonic.
//! - `combine` consumes a fully-signed (or junk-filled) PST and
//!   produces a consensus-core `Transaction` with on-chain
//!   sigscripts.

pub mod combine;
mod derive;
mod ecdsa;
pub mod error;
mod schnorr;
pub mod wire;

#[cfg(test)]
mod tests;

pub use combine::extract_transaction;
pub use error::SignError;

// Local-signing entry points consumed by the offline subcommand
// dispatchers (`sign`, plus the upcoming `send` and `bump-fee`).
pub use ecdsa::sign_pst_ecdsa_with_mnemonic;
pub use schnorr::sign_pst_schnorr_with_mnemonic;

/// True when every input of the PST has at least
/// `minimum_signatures` non-empty signatures.
pub fn is_pst_fully_signed(pst: &crate::serialization::wire::PartiallySignedTransaction) -> bool {
    for input in &pst.partially_signed_inputs {
        let n: u32 = input.pub_key_signature_pairs.iter().filter(|p| !p.signature.is_empty()).count() as u32;
        if n < input.minimum_signatures {
            return false;
        }
    }
    true
}
