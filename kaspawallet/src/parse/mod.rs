//! `parse` subcommand body. Decodes one or more hex-encoded
//! `PartiallySignedTransaction` messages and emits a human-
//! readable transcript.
//!
//! Output is plain text (not JSON). The transcript shape is the
//! contract exercised by the parity tests under
//! `kaspawallet/tests/`.
//!
//! The hex format supports MULTIPLE `PartiallySignedTransaction`
//! payloads concatenated with a `_` separator; this module
//! accepts the same multi-transaction form.
//!
//! Mass + fee-rate output: when the caller supplies a `KeysFile`,
//! the transcript trails with "Mass: N grams" and
//! "Fee rate: X Sompi/Gram" lines computed via
//! `crate::mass::estimate_mass_after_signatures`. When no keyfile
//! is supplied, those two lines are omitted; everything before
//! them is identical.

mod error;

#[cfg(test)]
mod tests;

use std::io::Write;

use kaspa_addresses::Prefix;
use kaspa_consensus_core::config::params::Params;
use kaspa_consensus_core::constants::SOMPI_PER_KASPA;
use kaspa_consensus_core::network::NetworkType;

use crate::cli::network::NetworkFlags;
use crate::keyfile::KeysFile;
use crate::mass::estimate_mass_after_signatures;
use crate::serialization::{deserialize_partially_signed_transaction, wire};
use crate::sign::SignError;
use crate::sign::wire::wire_to_consensus_tx;

pub use error::ParseError;

/// Separator used to concatenate multiple hex-encoded
/// transactions in one input string.
const HEX_TRANSACTIONS_SEPARATOR: char = '_';

/// Sources of the transaction hex. Exactly one MUST be `Some`;
/// passing both or neither yields a `ParseError`. When
/// `keysfile` is `Some`, the trailing "Mass: ... grams" and
/// "Fee rate: ... Sompi/Gram" lines are emitted; otherwise both
/// trailing lines are omitted.
#[derive(Debug)]
pub struct ParseInput<'a> {
    pub transaction: Option<&'a str>,
    pub transaction_file: Option<&'a str>,
    pub verbose: bool,
    pub network: &'a NetworkFlags,
    pub keysfile: Option<&'a KeysFile>,
}

/// Resolve the active address prefix from the parsed
/// `NetworkFlags`. The clap arg-group makes
/// `testnet`/`simnet`/`devnet` mutually exclusive; mainnet (no
/// flag) is the default.
pub fn resolve_prefix(network: &NetworkFlags) -> Prefix {
    match resolve_network_type(network) {
        NetworkType::Mainnet => Prefix::Mainnet,
        NetworkType::Testnet => Prefix::Testnet,
        NetworkType::Simnet => Prefix::Simnet,
        NetworkType::Devnet => Prefix::Devnet,
    }
}

/// Resolve the consensus `NetworkType` from the parsed
/// `NetworkFlags`.
pub fn resolve_network_type(network: &NetworkFlags) -> NetworkType {
    if network.simnet {
        NetworkType::Simnet
    } else if network.devnet {
        NetworkType::Devnet
    } else if network.testnet {
        NetworkType::Testnet
    } else {
        NetworkType::Mainnet
    }
}

/// Resolve the active `Params` from the parsed `NetworkFlags`.
/// Used for mass calculation when a keysfile is supplied.
fn resolve_params(network: &NetworkFlags) -> Params {
    Params::from(resolve_network_type(network))
}

/// Decode the hex input the caller supplies, run the per-PST
/// transcript render against the given writer. Returns the
/// number of PSTs decoded on success.
pub fn parse<W: Write>(input: &ParseInput<'_>, out: &mut W) -> Result<usize, ParseError> {
    let hex_text = resolve_hex_input(input)?;
    let trimmed = hex_text.trim();
    let prefix = resolve_prefix(input.network);
    let params = resolve_params(input.network);

    let mut count = 0;
    for (i, hex_chunk) in trimmed.split(HEX_TRANSACTIONS_SEPARATOR).enumerate() {
        let bytes = hex::decode(hex_chunk).map_err(|source| ParseError::InvalidHex { index: i, source })?;
        let pst = deserialize_partially_signed_transaction(&bytes).map_err(|source| ParseError::Serialization { index: i, source })?;
        render_one(&pst, i + 1, prefix, input.verbose, input.keysfile, &params, out)
            .map_err(|source| ParseError::Conversion { index: i, source })?;
        count += 1;
    }

    Ok(count)
}

fn resolve_hex_input(input: &ParseInput<'_>) -> Result<String, ParseError> {
    match (input.transaction, input.transaction_file) {
        (None, None) => Err(ParseError::MissingInput),
        (Some(_), Some(_)) => Err(ParseError::ConflictingInput),
        (Some(literal), None) => Ok(literal.to_owned()),
        (None, Some(path)) => std::fs::read_to_string(path).map_err(|source| ParseError::InputRead { path: path.to_owned(), source }),
    }
}

fn render_one<W: Write>(
    pst: &wire::PartiallySignedTransaction,
    one_based_index: usize,
    prefix: Prefix,
    verbose: bool,
    keysfile: Option<&KeysFile>,
    params: &Params,
    out: &mut W,
) -> Result<(), SignError> {
    let tx_msg = pst.tx.as_ref().ok_or(SignError::Missing("PartiallySignedTransaction.tx"))?;
    let consensus_tx = wire_to_consensus_tx(tx_msg)?;
    let tx_id = consensus_tx.id();

    let _ = writeln!(out, "Transaction #{one_based_index} ID: \t{tx_id}");
    let _ = writeln!(out);

    let mut all_input_sompi: u64 = 0;
    for (idx, input) in tx_msg.inputs.iter().enumerate() {
        let psi =
            pst.partially_signed_inputs.get(idx).ok_or(SignError::Missing("PartiallySignedTransaction.partiallySignedInputs[idx]"))?;
        let prev = psi.prev_output.as_ref().ok_or(SignError::Missing("PartiallySignedInput.prevOutput"))?;
        all_input_sompi = all_input_sompi.saturating_add(prev.value);

        if verbose {
            let outpoint = input.previous_outpoint.as_ref().ok_or(SignError::Missing("tx.input.previousOutpoint"))?;
            let txid_bytes = outpoint.transaction_id.as_ref().ok_or(SignError::Missing("tx.input.previousOutpoint.transactionId"))?;
            let _ = writeln!(
                out,
                "Input {idx}: \tOutpoint: {}:{} \tAmount: {:.2} Kaspa",
                hex::encode(&txid_bytes.bytes),
                outpoint.index,
                kaspa_amount(prev.value),
            );
        }
    }
    if verbose {
        let _ = writeln!(out);
    }

    let mut all_output_sompi: u64 = 0;
    for (idx, output) in tx_msg.outputs.iter().enumerate() {
        let spk_msg = output.script_public_key.as_ref().ok_or(SignError::Missing("tx.output.scriptPublicKey"))?;
        let address_string = render_script_public_key(spk_msg, prefix);
        let _ = writeln!(out, "Output {idx}: \tRecipient: {address_string} \tAmount: {:.2} Kaspa", kaspa_amount(output.value));
        all_output_sompi = all_output_sompi.saturating_add(output.value);
    }
    let _ = writeln!(out);

    let fee = all_input_sompi.saturating_sub(all_output_sompi);
    let _ = writeln!(out, "Fee:\t{fee} Sompi ({} KAS)", kaspa_amount_long(fee));

    if let Some(kf) = keysfile {
        let mass = estimate_mass_after_signatures(pst, params, kf.ecdsa)?;
        let _ = writeln!(out, "Mass: {mass} grams");
        let fee_rate = if mass == 0 { 0.0 } else { fee as f64 / mass as f64 };
        let _ = writeln!(out, "Fee rate: {fee_rate:.2} Sompi/Gram");
    }
    Ok(())
}

/// Render a script-public-key the way the reference does: either
/// the address string for a standard script type, or the
/// `<Non-standard transaction script public key: HEX>`
/// placeholder.
fn render_script_public_key(spk: &wire::ScriptPublicKey, prefix: Prefix) -> String {
    let Ok(version) = u16::try_from(spk.version) else {
        return non_standard_placeholder(&spk.script);
    };
    let consensus_spk = kaspa_consensus_core::tx::ScriptPublicKey::from_vec(version, spk.script.clone());
    match kaspa_txscript::extract_script_pub_key_address(&consensus_spk, prefix) {
        Ok(addr) => addr.to_string(),
        Err(_) => non_standard_placeholder(&spk.script),
    }
}

fn non_standard_placeholder(script: &[u8]) -> String {
    format!("<Non-standard transaction script public key: {}>", hex::encode(script))
}

/// `%.2f Kaspa` rendering used for input / output lines.
fn kaspa_amount(sompi: u64) -> f64 {
    sompi as f64 / SOMPI_PER_KASPA as f64
}

/// `%f KAS` rendering for the fee summary line: six-decimal
/// precision, distinct from the two-decimal precision used on
/// input / output lines.
fn kaspa_amount_long(sompi: u64) -> String {
    format!("{:.6}", sompi as f64 / SOMPI_PER_KASPA as f64)
}
