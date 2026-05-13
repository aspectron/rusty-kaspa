//! Cosigner-prefix-then-relative-path BIP-32 derivation helper
//! shared by the Schnorr and ECDSA single-cosigner sign modules.
//!
//! Reference: [BIP-32](https://github.com/bitcoin/bips/blob/master/bip-0032.mediawiki).
//!
//! The derivation chain is:
//!
//! 1. Walk the master xpriv to `m/<purpose>'/<COIN_TYPE>'/0'`.
//! 2. Extend by the per-input `DerivationPath` from the PST.
//! 3. Extract the public half and match against the
//!    `PubKeySignaturePair.ExtendedPublicKey`.
//!
//! Two layout invariants the rest of the wallet depends on:
//!
//! - `PubKeySignaturePair.ExtendedPublicKey` stores the LEAF-level
//!   xpub.
//! - `PartiallySignedInput.DerivationPath` is RELATIVE to the
//!   cosigner prefix `m/<purpose>'/<COIN_TYPE>'/0'`.

use std::str::FromStr;

use kaspa_bip32::{DerivationPath, ExtendedPrivateKey, ExtendedPublicKey, Prefix, SecretKey};

use super::error::SignError;
use crate::serialization::wire;

/// BIP-43 purpose component for single-signer wallets.
const SINGLE_SIGNER_PURPOSE: u32 = 44;

/// BIP-43-style purpose component for multisig wallets.
const MULTISIG_PURPOSE: u32 = 45;

/// Kaspa SLIP-0044 coin-type component.
const COIN_TYPE: u32 = 111111;

/// Length of the textual version prefix in a serialized extended
/// public key (`kpub`, `ktub`, etc.).
const XPUB_PREFIX_LEN: usize = 4;

/// Derive the leaf signing key for a partially-signed input and
/// locate the matching `PubKeySignaturePair`. The chain runs:
///
/// 1. Walk the master xpriv to `m/<purpose>'/<COIN_TYPE>'/0'`,
///    where `purpose` is 44 for single-signer (one pair) or 45
///    for multisig (more than one pair).
/// 2. Walk the resulting cosigner xpriv via the input's relative
///    derivation path (e.g. `m/0/0`) to the leaf xpriv.
/// 3. Match the leaf xpub's serialized form against every stored
///    pair's `extended_pub_key`, reusing each pair's own textual
///    prefix to drive the version bytes (so a `ktub`-stored pair
///    compares with a `ktub`-encoded leaf, a `kpub` pair with a
///    `kpub`-encoded leaf, etc. -- network selection flows through
///    the user-supplied cosigner xpub).
pub(super) fn derive_leaf_and_match_pair(
    master: &ExtendedPrivateKey<SecretKey>,
    psi: &wire::PartiallySignedInput,
    input_index: usize,
) -> Result<(ExtendedPrivateKey<SecretKey>, usize), SignError> {
    let is_multisig = psi.pub_key_signature_pairs.len() > 1;
    let purpose = if is_multisig { MULTISIG_PURPOSE } else { SINGLE_SIGNER_PURPOSE };
    let cosigner_default_path = DerivationPath::from_str(&format!("m/{purpose}'/{COIN_TYPE}'/0'"))?;
    let cosigner = master.clone().derive_path(&cosigner_default_path)?;

    let relative_path = DerivationPath::from_str(&psi.derivation_path)?;
    let leaf = cosigner.derive_path(&relative_path)?;
    let leaf_xpub: ExtendedPublicKey<secp256k1::PublicKey> = (&leaf).into();

    let pair_index = psi
        .pub_key_signature_pairs
        .iter()
        .position(|pair| pair_matches_leaf(&leaf_xpub, &pair.extended_pub_key))
        .ok_or(SignError::CosignerMismatch { input_index })?;

    Ok((leaf, pair_index))
}

fn pair_matches_leaf(leaf_xpub: &ExtendedPublicKey<secp256k1::PublicKey>, pair_xpub: &str) -> bool {
    let Some(prefix_str) = pair_xpub.get(..XPUB_PREFIX_LEN) else { return false };
    let Ok(prefix) = Prefix::try_from(prefix_str) else { return false };
    leaf_xpub.to_string(Some(prefix)) == pair_xpub
}
