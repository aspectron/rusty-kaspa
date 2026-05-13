//! `sweep` subcommand.
//!
//! Mass-aware sweep of all UTXOs controlled by a Schnorr private
//! key into a daemon-owned recipient address. The dispatcher:
//!
//! 1. Derives the pay-to-pubkey address from the supplied private
//!    key (hex-encoded 32-byte secp256k1 scalar).
//! 2. Queries the wallet daemon for the address's external
//!    spendable UTXOs (`GetExternalSpendableUTXOs`).
//! 3. Asks the daemon for a fresh wallet-owned recipient address
//!    (`NewAddress`).
//! 4. Splits the UTXOs across one or more transactions whose
//!    compute mass stays under the standard mempool ceiling.
//! 5. Signs every input with the supplied Schnorr key against
//!    `SIG_HASH_ALL`.
//! 6. Broadcasts each transaction via the daemon's `Broadcast`
//!    surface with `is_domain: true` (the daemon decodes the
//!    payload as a `protoserialization.TransactionMessage`).
//! 7. Prints per-transaction txid + swept amount and a grand-
//!    total summary.
//!
//! Reference: the operator-visible output and the splitting
//! algorithm follow
//! https://github.com/kaspanet/kaspad/blob/master/cmd/kaspawallet/sweep.go
//! (`sweep`, `createSplitTransactionsWithSchnorrPrivteKey`,
//! `signWithSchnorrPrivateKey`); preserving the line-by-line
//! output shape is load-bearing for Phase 9 cross-binary parity
//! tests against the reference binary.

use std::process::ExitCode;
use std::str::FromStr;

use kaspa_addresses::{Address, Version};
use kaspa_consensus_core::Hash;
use kaspa_consensus_core::config::params::Params;
use kaspa_consensus_core::constants::{MAX_TX_IN_SEQUENCE_NUM, TX_VERSION};
use kaspa_consensus_core::hashing::sighash::{SigHashReusedValuesUnsync, calc_schnorr_signature_hash};
use kaspa_consensus_core::hashing::sighash_type::SIG_HASH_ALL;
use kaspa_consensus_core::mass::MassCalculator;
use kaspa_consensus_core::subnets::SUBNETWORK_ID_NATIVE;
use kaspa_consensus_core::tx::{
    ScriptPublicKey, SignableTransaction, Transaction, TransactionInput, TransactionOutpoint, TransactionOutput, UtxoEntry,
};
use kaspa_txscript::pay_to_address_script;
use kaspa_txscript::script_builder::ScriptBuilder;
use kaspa_wallet_grpc_client::{
    ClientOptions, connect,
    kaspawalletd::{BroadcastRequest, GetExternalSpendableUtxOsRequest, NewAddressRequest, UtxosByAddressesEntry},
};
use secp256k1::{Keypair, Message, SECP256K1, SecretKey};

use crate::cli::args::SweepArgs;
use crate::cli::dispatch::{build_runtime, endpoint_url, fail, format_kas, network_type};
use crate::cli::network::NetworkFlags;
use crate::serialization::wire;

/// Fee charged per input on every sweep-built transaction.
/// Source: kaspad `cmd/kaspawallet/sweep.go:27`
/// (`const feePerInput = 10000`).
const SWEEP_FEE_PER_INPUT: u64 = 10_000;

/// Reserved compute-mass budget for the eventual per-input
/// Schnorr signatures the splitting loop's running transaction
/// will accrue at signing time. The loop subtracts this from the
/// available mass budget BEFORE comparing to the ceiling so the
/// finished tx still fits after signatures are pushed. Source:
/// kaspad `cmd/kaspawallet/sweep.go:149`
/// (`extraMass := uint64(7000)`).
const SWEEP_EXTRA_MASS_FOR_SIGNATURES: u64 = 7000;

/// Standard mempool transaction-mass ceiling. Defined locally so
/// this module does not pull in `kaspa-wallet-core` as a runtime
/// dependency for a single constant. Source: rusty-kaspa
/// `mining/src/mempool/check_transaction_standard.rs:38` (canonical
/// private constant) and the equivalent public re-export at
/// `wallet/core/src/tx/mass.rs:24`. The value matches the kaspad
/// reference (`domain/miningmanager/mempool.MaximumStandardTransactionMass`).
const MAXIMUM_STANDARD_TRANSACTION_MASS: u64 = 100_000;

/// Length of a serialised Schnorr signature in bytes (BIP-340).
const SCHNORR_SIGNATURE_LEN: usize = 64;

/// secp256k1 private-key length in bytes.
const PRIVATE_KEY_LEN: usize = 32;

/// Per-input `sig_op_count` carried by every sweep-built input.
/// Pay-to-pubkey single-Schnorr-key inputs do exactly one
/// signature-op; the value is load-bearing for sighash
/// computation (the sighash routine folds the sig-op count into
/// the digest) and for the mass calculator's per-input sig-op
/// term.
const SWEEP_INPUT_SIG_OP_COUNT: u8 = 1;

#[derive(Debug, thiserror::Error)]
pub enum SweepError {
    #[error("'sweep' requires --private-key (hex-encoded 32-byte secp256k1 private key)")]
    PrivateKeyMissing,

    #[error("'sweep': --private-key hex decode failed: {0}")]
    PrivateKeyHexDecode(String),

    #[error("'sweep': --private-key must be {expected} bytes (got {actual} bytes)")]
    PrivateKeyLength { expected: usize, actual: usize },

    #[error("'sweep': invalid secp256k1 private key: {0}")]
    PrivateKeyInvalid(String),

    #[error("Could not find any spendable UTXOs in {0}")]
    NoSpendableUtxos(String),

    #[error("invalid recipient address from daemon: {0}")]
    RecipientAddress(String),

    #[error("UTXO entry from daemon is malformed: {0}")]
    UtxoEntryMalformed(String),

    #[error("transaction with one input and one output violates transaction mass")]
    SingleInputOverflow,

    #[error("script-builder error: {0}")]
    ScriptBuilder(String),

    #[error("sighash digest is not a valid secp256k1 message: {0}")]
    SighashMessage(String),
}

/// Validate the operator-supplied hex private key and produce a
/// secp256k1 `SecretKey`. Errors mirror the
/// reference binary's exit-1 messages so the cross-binary parity
/// tests pass.
fn parse_private_key(hex_str: &str) -> Result<SecretKey, SweepError> {
    if hex_str.is_empty() {
        return Err(SweepError::PrivateKeyMissing);
    }
    let bytes = hex::decode(hex_str).map_err(|e| SweepError::PrivateKeyHexDecode(e.to_string()))?;
    if bytes.len() != PRIVATE_KEY_LEN {
        return Err(SweepError::PrivateKeyLength { expected: PRIVATE_KEY_LEN, actual: bytes.len() });
    }
    SecretKey::from_slice(&bytes).map_err(|e| SweepError::PrivateKeyInvalid(e.to_string()))
}

/// Derive the pay-to-pubkey address (Schnorr x-only) for the
/// supplied private key on the active network.
fn address_from_private_key(secret: &SecretKey, network: &NetworkFlags) -> Address {
    let (xonly, _parity) = Keypair::from_secret_key(SECP256K1, secret).x_only_public_key();
    Address::new(network.address_prefix(), Version::PubKey, &xonly.serialize())
}

/// Lift one daemon-returned UTXO entry into a consensus-friendly
/// `(outpoint, entry)` pair.
fn lift_utxo(entry: &UtxosByAddressesEntry) -> Result<(TransactionOutpoint, UtxoEntry), SweepError> {
    let outpoint = entry.outpoint.as_ref().ok_or_else(|| SweepError::UtxoEntryMalformed("missing outpoint".to_owned()))?;
    let txid = Hash::from_str(&outpoint.transaction_id)
        .map_err(|e| SweepError::UtxoEntryMalformed(format!("transaction_id hex decode: {e}")))?;
    let utxo = entry.utxo_entry.as_ref().ok_or_else(|| SweepError::UtxoEntryMalformed("missing utxo_entry".to_owned()))?;
    let script =
        utxo.script_public_key.as_ref().ok_or_else(|| SweepError::UtxoEntryMalformed("missing script_public_key".to_owned()))?;
    let version: u16 = u16::try_from(script.version)
        .map_err(|_| SweepError::UtxoEntryMalformed(format!("script version {} exceeds u16::MAX", script.version)))?;
    let script_bytes = hex::decode(&script.script_public_key)
        .map_err(|e| SweepError::UtxoEntryMalformed(format!("script_public_key hex decode: {e}")))?;
    let script_public_key = ScriptPublicKey::from_vec(version, script_bytes);
    let consensus_entry = UtxoEntry::new(utxo.amount, script_public_key, utxo.block_daa_score, utxo.is_coinbase);
    Ok((TransactionOutpoint::new(txid, outpoint.index), consensus_entry))
}

/// Build the unsigned split transactions paying `to_address` from
/// `utxos`. The output shape mirrors the reference splitting
/// algorithm: accumulate inputs into the running transaction
/// until the next-input addition would push compute-mass plus the
/// reserved signature budget over the standard ceiling; on
/// overflow, commit the last-valid transaction and start a fresh
/// one. The final transaction (containing any residual inputs)
/// is committed on loop exit.
fn build_split_transactions(
    params: &Params,
    utxos: &[(TransactionOutpoint, UtxoEntry)],
    to_address: &Address,
    fee_per_input: u64,
) -> Result<Vec<Transaction>, SweepError> {
    let script_public_key = pay_to_address_script(to_address);
    let mut splits: Vec<Transaction> = Vec::new();
    let mass_calc = MassCalculator::new_with_consensus_params(params);

    let mut last_valid_inputs: Vec<TransactionInput> = Vec::new();
    let mut last_valid_output_value: u64 = 0;
    let mut current_inputs: Vec<TransactionInput> = Vec::new();
    let mut current_total: u64 = 0;

    for (i, (outpoint, entry)) in utxos.iter().enumerate() {
        current_total = current_total.saturating_add(entry.amount);
        current_inputs.push(TransactionInput::new(*outpoint, Vec::new(), MAX_TX_IN_SEQUENCE_NUM, SWEEP_INPUT_SIG_OP_COUNT));

        let output_value = current_total.saturating_sub(fee_per_input.saturating_mul(current_inputs.len() as u64));
        let tx = build_tx(current_inputs.clone(), output_value, script_public_key.clone());
        let mass = mass_calc.calc_non_contextual_masses(&tx).compute_mass;

        if mass.saturating_add(SWEEP_EXTRA_MASS_FOR_SIGNATURES) >= MAXIMUM_STANDARD_TRANSACTION_MASS {
            if current_inputs.len() == 1 {
                return Err(SweepError::SingleInputOverflow);
            }
            let committed = build_tx(std::mem::take(&mut last_valid_inputs), last_valid_output_value, script_public_key.clone());
            splits.push(committed);

            current_inputs = vec![TransactionInput::new(*outpoint, Vec::new(), MAX_TX_IN_SEQUENCE_NUM, SWEEP_INPUT_SIG_OP_COUNT)];
            current_total = entry.amount;
            last_valid_inputs.clone_from(&current_inputs);
            last_valid_output_value = current_total.saturating_sub(fee_per_input.saturating_mul(current_inputs.len() as u64));
            continue;
        }

        if i == utxos.len() - 1 {
            let finished = build_tx(std::mem::take(&mut current_inputs), output_value, script_public_key.clone());
            splits.push(finished);
            break;
        }

        last_valid_inputs.clone_from(&current_inputs);
        last_valid_output_value = output_value;
    }

    Ok(splits)
}

/// Assemble a single sweep-shape transaction with one output
/// paying `output_value` to `script_public_key`.
fn build_tx(inputs: Vec<TransactionInput>, output_value: u64, script_public_key: ScriptPublicKey) -> Transaction {
    let outputs = vec![TransactionOutput::new(output_value, script_public_key)];
    Transaction::new(TX_VERSION, inputs, outputs, 0, SUBNETWORK_ID_NATIVE, 0, Vec::new())
}

/// Sign every input of every split transaction with the supplied
/// Schnorr private key. The post-condition is that every
/// `transactions[i].inputs[j].signature_script` is the PUSHDATA-
/// framed 65-byte signature blob (64 sig bytes + 1 `SIG_HASH_ALL`
/// byte) the consensus interpreter expects on the wire.
fn sign_split_transactions(
    secret: &SecretKey,
    transactions: Vec<Transaction>,
    source_entries: &[UtxoEntry],
) -> Result<Vec<Transaction>, SweepError> {
    let keypair = Keypair::from_secret_key(SECP256K1, secret);
    let reused = SigHashReusedValuesUnsync::new();
    let mut signed: Vec<Transaction> = Vec::with_capacity(transactions.len());
    let mut entry_cursor: usize = 0;

    for tx in transactions {
        let input_count = tx.inputs.len();
        let entries_slice: &[UtxoEntry] = &source_entries[entry_cursor..entry_cursor + input_count];
        entry_cursor += input_count;
        let signable = SignableTransaction::with_entries(tx, entries_slice.to_vec());

        let scripts: Vec<Vec<u8>> = (0..input_count)
            .map(|input_index| {
                let sighash = calc_schnorr_signature_hash(&signable.as_verifiable(), input_index, SIG_HASH_ALL, &reused);
                let message = Message::from_digest_slice(sighash.as_bytes().as_slice())
                    .map_err(|e| SweepError::SighashMessage(e.to_string()))?;
                let signature: [u8; SCHNORR_SIGNATURE_LEN] = *keypair.sign_schnorr(message).as_ref();
                let mut sig_with_hashtype = Vec::with_capacity(SCHNORR_SIGNATURE_LEN + 1);
                sig_with_hashtype.extend_from_slice(&signature);
                sig_with_hashtype.push(SIG_HASH_ALL.to_u8());
                let script =
                    ScriptBuilder::new().add_data(&sig_with_hashtype).map_err(|e| SweepError::ScriptBuilder(e.to_string()))?.drain();
                Ok::<Vec<u8>, SweepError>(script)
            })
            .collect::<Result<Vec<_>, _>>()?;

        let mut tx = signable.tx;
        for (input, script) in tx.inputs.iter_mut().zip(scripts) {
            input.signature_script = script;
        }
        signed.push(tx);
    }

    Ok(signed)
}

/// Convert a consensus-core `Transaction` into the proto-wire
/// `TransactionMessage` the daemon's `Broadcast` handler decodes
/// when the request is flagged `is_domain: true`. Symmetric to
/// the existing `wire_to_consensus_tx` lift in
/// `crate::sign::wire`.
fn consensus_tx_to_wire_message(tx: &Transaction) -> wire::TransactionMessage {
    let subnetwork_bytes: &[u8] = tx.subnetwork_id.as_ref();
    wire::TransactionMessage {
        version: tx.version as u32,
        inputs: tx.inputs.iter().map(consensus_input_to_wire).collect(),
        outputs: tx.outputs.iter().map(consensus_output_to_wire).collect(),
        lock_time: tx.lock_time,
        subnetwork_id: Some(wire::SubnetworkId { bytes: subnetwork_bytes.to_vec() }),
        gas: tx.gas,
        payload: tx.payload.clone(),
    }
}

fn consensus_input_to_wire(input: &TransactionInput) -> wire::TransactionInput {
    wire::TransactionInput {
        previous_outpoint: Some(wire::Outpoint {
            transaction_id: Some(wire::TransactionId { bytes: input.previous_outpoint.transaction_id.as_bytes().to_vec() }),
            index: input.previous_outpoint.index,
        }),
        signature_script: input.signature_script.clone(),
        sequence: input.sequence,
        sig_op_count: input.sig_op_count as u32,
    }
}

fn consensus_output_to_wire(output: &TransactionOutput) -> wire::TransactionOutput {
    wire::TransactionOutput {
        value: output.value,
        script_public_key: Some(wire::ScriptPublicKey {
            script: output.script_public_key.script().to_vec(),
            version: output.script_public_key.version() as u32,
        }),
    }
}

/// Top-level orchestration: parse args, dial daemon, query UTXOs,
/// build + sign split transactions, broadcast, print summary.
pub fn run_sweep(args: SweepArgs, top: &NetworkFlags) -> ExitCode {
    let mut network = top.clone();
    network.combine(&args.network);

    let private_key_hex = match args.private_key.as_deref() {
        Some(s) => s,
        None => return fail(SweepError::PrivateKeyMissing.to_string()),
    };
    let secret = match parse_private_key(private_key_hex) {
        Ok(s) => s,
        Err(e) => return fail(e.to_string()),
    };
    let source_address = address_from_private_key(&secret, &network);

    let runtime = match build_runtime() {
        Ok(r) => r,
        Err(e) => return e,
    };

    runtime.block_on(async move {
        let mut client = match connect(endpoint_url(&args.daemon_address), ClientOptions::default()).await {
            Ok(c) => c,
            Err(e) => return fail(format!("dial daemon '{}': {e}", args.daemon_address)),
        };

        let resp =
            match client.get_external_spendable_utx_os(GetExternalSpendableUtxOsRequest { address: source_address.to_string() }).await
            {
                Ok(r) => r.into_inner(),
                Err(s) => return fail(format!("GetExternalSpendableUTXOs failed: {s}")),
            };
        if resp.entries.is_empty() {
            return fail(SweepError::NoSpendableUtxos(source_address.to_string()).to_string());
        }

        let outpoints_and_entries = match resp.entries.iter().map(lift_utxo).collect::<Result<Vec<_>, _>>() {
            Ok(v) => v,
            Err(e) => return fail(e.to_string()),
        };
        let source_entries: Vec<UtxoEntry> = outpoints_and_entries.iter().map(|(_, e)| e.clone()).collect();

        let new_address_resp = match client.new_address(NewAddressRequest {}).await {
            Ok(r) => r.into_inner(),
            Err(s) => return fail(format!("NewAddress failed: {s}")),
        };
        let recipient = match Address::try_from(new_address_resp.address.as_str()) {
            Ok(a) => a,
            Err(e) => return fail(SweepError::RecipientAddress(e.to_string()).to_string()),
        };

        let params = Params::from(network_type(&network));
        let splits = match build_split_transactions(&params, &outpoints_and_entries, &recipient, SWEEP_FEE_PER_INPUT) {
            Ok(v) => v,
            Err(e) => return fail(e.to_string()),
        };
        let signed = match sign_split_transactions(&secret, splits, &source_entries) {
            Ok(v) => v,
            Err(e) => return fail(e.to_string()),
        };

        let wire_bytes: Vec<Vec<u8>> = signed.iter().map(|tx| consensus_tx_to_wire_message(tx).encode_to_vec()).collect();

        println!("\nSweeping...");
        println!("\tFrom:\t {source_address}");
        println!("\tTo:\t {recipient}");

        let broadcast_resp = match client.broadcast(BroadcastRequest { is_domain: true, transactions: wire_bytes }).await {
            Ok(r) => r.into_inner(),
            Err(s) => return fail(format!("Broadcast failed: {s}")),
        };

        let mut total_extracted: u64 = 0;
        println!("\nTransaction ID(s):");
        for (txid, tx) in broadcast_resp.tx_ids.iter().zip(signed.iter()) {
            let swept_value = tx.outputs.first().map(|o| o.value).unwrap_or(0);
            println!("\t{txid}");
            println!("\tSwept:\t {}  KAS", format_kas(swept_value));
            total_extracted = total_extracted.saturating_add(swept_value);
        }
        println!("\nTotal Funds swept (including transaction fees):");
        println!("\t {}  KAS", format_kas(total_extracted));

        ExitCode::SUCCESS
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic_outpoint(index: u32) -> TransactionOutpoint {
        let mut bytes = [0u8; 32];
        bytes[0] = index as u8;
        bytes[1] = (index >> 8) as u8;
        TransactionOutpoint::new(Hash::from_bytes(bytes), index)
    }

    fn synthetic_entry(amount: u64) -> UtxoEntry {
        let mut spk_bytes = vec![0u8; 34];
        spk_bytes[0] = 0x20;
        spk_bytes[33] = 0xac;
        let script_public_key = ScriptPublicKey::from_vec(0, spk_bytes);
        UtxoEntry::new(amount, script_public_key, 0, false)
    }

    fn synthetic_pair(index: u32, amount: u64) -> (TransactionOutpoint, UtxoEntry) {
        (synthetic_outpoint(index), synthetic_entry(amount))
    }

    fn synthetic_recipient(network: &NetworkFlags) -> Address {
        Address::new(network.address_prefix(), Version::PubKey, &[0xab; 32])
    }

    fn private_key_fixture() -> SecretKey {
        let mut bytes = [0u8; PRIVATE_KEY_LEN];
        bytes[31] = 1;
        SecretKey::from_slice(&bytes).expect("non-zero fixed scalar is a valid secp256k1 secret")
    }

    #[test]
    fn test_parse_private_key_rejects_empty_string() {
        let err = parse_private_key("").expect_err("empty string fails");
        assert!(matches!(err, SweepError::PrivateKeyMissing));
    }

    #[test]
    fn test_parse_private_key_rejects_non_hex() {
        let err = parse_private_key("zzzz").expect_err("non-hex fails");
        assert!(matches!(err, SweepError::PrivateKeyHexDecode(_)));
    }

    #[test]
    fn test_parse_private_key_rejects_wrong_length() {
        let err = parse_private_key(&"ab".repeat(31)).expect_err("31-byte input fails");
        assert!(matches!(err, SweepError::PrivateKeyLength { expected: 32, actual: 31 }));
    }

    #[test]
    fn test_parse_private_key_accepts_thirty_two_bytes() {
        let hex_str = "0".repeat(63) + "1";
        let secret = parse_private_key(&hex_str).expect("valid 32-byte hex accepted");
        let bytes = secret.secret_bytes();
        assert_eq!(bytes[31], 1);
        assert_eq!(bytes[..31], [0u8; 31]);
    }

    #[test]
    fn test_address_from_private_key_uses_network_prefix() {
        let secret = private_key_fixture();
        let network = NetworkFlags { testnet: true, ..NetworkFlags::default() };
        let addr = address_from_private_key(&secret, &network);
        assert_eq!(addr.prefix, kaspa_addresses::Prefix::Testnet);
        assert_eq!(addr.version, Version::PubKey);
        assert_eq!(addr.payload.len(), 32);
    }

    #[test]
    fn test_build_split_transactions_packs_few_utxos_into_single_tx() {
        let network = NetworkFlags::default();
        let params = Params::from(network_type(&network));
        let recipient = synthetic_recipient(&network);
        let utxos = vec![synthetic_pair(0, 1_000_000), synthetic_pair(1, 2_000_000)];
        let splits = build_split_transactions(&params, &utxos, &recipient, SWEEP_FEE_PER_INPUT).expect("two small inputs fit one tx");
        assert_eq!(splits.len(), 1, "two small inputs must build a single split tx");
        let tx = &splits[0];
        assert_eq!(tx.inputs.len(), 2);
        assert_eq!(tx.outputs.len(), 1);
        let expected_value = 1_000_000u64 + 2_000_000 - 2 * SWEEP_FEE_PER_INPUT;
        assert_eq!(tx.outputs[0].value, expected_value, "single-tx output = total - fee_per_input * input_count");
        for input in &tx.inputs {
            assert_eq!(input.sig_op_count, SWEEP_INPUT_SIG_OP_COUNT);
            assert_eq!(input.sequence, MAX_TX_IN_SEQUENCE_NUM);
            assert!(input.signature_script.is_empty(), "unsigned tx has empty sigscript");
        }
    }

    #[test]
    fn test_build_split_transactions_splits_on_mass_overflow() {
        let network = NetworkFlags::default();
        let params = Params::from(network_type(&network));
        let recipient = synthetic_recipient(&network);
        let pair_count = 600usize;
        let mut utxos = Vec::with_capacity(pair_count);
        for i in 0..pair_count {
            utxos.push(synthetic_pair(i as u32, 1_000_000_000));
        }
        let splits = build_split_transactions(&params, &utxos, &recipient, SWEEP_FEE_PER_INPUT)
            .expect("split-loop succeeds on synthetic large set");
        assert!(splits.len() > 1, "many inputs force splitting (got {} splits)", splits.len());
        let mass_calc = MassCalculator::new_with_consensus_params(&params);
        for (i, tx) in splits.iter().enumerate() {
            let mass = mass_calc.calc_non_contextual_masses(tx).compute_mass;
            assert!(
                mass + SWEEP_EXTRA_MASS_FOR_SIGNATURES < MAXIMUM_STANDARD_TRANSACTION_MASS,
                "split #{i} mass {mass} + extra {SWEEP_EXTRA_MASS_FOR_SIGNATURES} must stay under the ceiling \
                 {MAXIMUM_STANDARD_TRANSACTION_MASS}"
            );
        }
        let total_inputs: usize = splits.iter().map(|t| t.inputs.len()).sum();
        assert_eq!(total_inputs, pair_count, "every input must land in exactly one split");
    }

    #[test]
    fn test_sign_split_transactions_populates_every_signature_script() {
        let network = NetworkFlags::default();
        let params = Params::from(network_type(&network));
        let secret = private_key_fixture();
        let pay_addr = address_from_private_key(&secret, &network);
        let script_public_key = pay_to_address_script(&pay_addr);
        let entry = UtxoEntry::new(5_000_000, script_public_key, 0, false);
        let outpoints = vec![(synthetic_outpoint(7), entry.clone()), (synthetic_outpoint(8), entry.clone())];
        let entries: Vec<UtxoEntry> = outpoints.iter().map(|(_, e)| e.clone()).collect();
        let recipient = synthetic_recipient(&network);
        let unsigned = build_split_transactions(&params, &outpoints, &recipient, SWEEP_FEE_PER_INPUT)
            .expect("synthetic two-input batch builds a single tx");
        assert_eq!(unsigned.len(), 1);
        let signed = sign_split_transactions(&secret, unsigned, &entries).expect("signing succeeds");
        assert_eq!(signed.len(), 1);
        let tx = &signed[0];
        for (idx, input) in tx.inputs.iter().enumerate() {
            // PUSHDATA opcode OP_DATA_65 (0x41) + 64-byte sig + 1-byte sighash type.
            assert_eq!(input.signature_script.len(), 1 + SCHNORR_SIGNATURE_LEN + 1, "input #{idx} sigscript length");
            assert_eq!(input.signature_script[0], 0x41, "input #{idx} starts with OP_DATA_65 push opcode");
            assert_eq!(
                *input.signature_script.last().unwrap(),
                SIG_HASH_ALL.to_u8(),
                "input #{idx} sigscript ends with SIG_HASH_ALL byte"
            );
        }
    }

    #[test]
    fn test_consensus_tx_to_wire_message_round_trips_through_existing_lift() {
        use crate::sign::wire::wire_to_consensus_tx;
        let network = NetworkFlags::default();
        let recipient = synthetic_recipient(&network);
        let script_public_key = pay_to_address_script(&recipient);
        let input = TransactionInput::new(synthetic_outpoint(42), vec![1, 2, 3], MAX_TX_IN_SEQUENCE_NUM, SWEEP_INPUT_SIG_OP_COUNT);
        let output = TransactionOutput::new(987_654, script_public_key);
        let tx = Transaction::new(TX_VERSION, vec![input], vec![output], 0, SUBNETWORK_ID_NATIVE, 0, vec![]);
        let wire_msg = consensus_tx_to_wire_message(&tx);
        let lifted = wire_to_consensus_tx(&wire_msg).expect("wire lift succeeds");
        assert_eq!(lifted.version, tx.version);
        assert_eq!(lifted.lock_time, tx.lock_time);
        assert_eq!(lifted.gas, tx.gas);
        assert_eq!(lifted.subnetwork_id, tx.subnetwork_id);
        assert_eq!(lifted.payload, tx.payload);
        assert_eq!(lifted.inputs.len(), tx.inputs.len());
        for (a, b) in lifted.inputs.iter().zip(&tx.inputs) {
            assert_eq!(a.previous_outpoint, b.previous_outpoint);
            assert_eq!(a.signature_script, b.signature_script);
            assert_eq!(a.sequence, b.sequence);
            assert_eq!(a.sig_op_count, b.sig_op_count);
        }
        assert_eq!(lifted.outputs.len(), tx.outputs.len());
        for (a, b) in lifted.outputs.iter().zip(&tx.outputs) {
            assert_eq!(a.value, b.value);
            assert_eq!(a.script_public_key, b.script_public_key);
        }
    }

    #[test]
    fn test_lift_utxo_decodes_hex_script_and_txid() {
        let entry = UtxosByAddressesEntry {
            address: "kaspa:qrxx".to_owned(),
            outpoint: Some(kaspa_wallet_grpc_client::kaspawalletd::Outpoint { transaction_id: "00".repeat(32), index: 3 }),
            utxo_entry: Some(kaspa_wallet_grpc_client::kaspawalletd::UtxoEntry {
                amount: 555,
                script_public_key: Some(kaspa_wallet_grpc_client::kaspawalletd::ScriptPublicKey {
                    version: 0,
                    script_public_key: "20".to_owned() + &"ab".repeat(32) + "ac",
                }),
                block_daa_score: 1234,
                is_coinbase: false,
            }),
        };
        let (outpoint, lifted) = lift_utxo(&entry).expect("well-formed entry lifts");
        assert_eq!(outpoint.transaction_id.as_bytes(), [0u8; 32]);
        assert_eq!(outpoint.index, 3);
        assert_eq!(lifted.amount, 555);
        assert_eq!(lifted.script_public_key.script().len(), 34);
        assert_eq!(lifted.script_public_key.script()[0], 0x20);
        assert_eq!(*lifted.script_public_key.script().last().unwrap(), 0xac);
    }
}
