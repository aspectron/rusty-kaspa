//! Unit tests for the parse subcommand body.

use std::path::PathBuf;

use super::{HEX_TRANSACTIONS_SEPARATOR, ParseError, ParseInput, parse, resolve_prefix};
use crate::cli::network::NetworkFlags;

fn fixture(name: &str) -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests");
    p.push("fixtures");
    p.push(name);
    p
}

fn reference_pst_hex() -> String {
    std::fs::read_to_string(fixture("go_emitted_pst.hex")).unwrap().trim().to_owned()
}

fn empty_network() -> NetworkFlags {
    NetworkFlags::default()
}

#[test]
fn test_parse_rejects_both_transaction_and_file() {
    let net = empty_network();
    let input =
        ParseInput { transaction: Some("aa"), transaction_file: Some("/tmp/x"), verbose: false, network: &net, keysfile: None };
    let mut buf = Vec::new();
    match parse(&input, &mut buf) {
        Err(ParseError::ConflictingInput) => {}
        other => panic!("expected ConflictingInput, got {other:?}"),
    }
}

#[test]
fn test_parse_rejects_missing_input() {
    let net = empty_network();
    let input = ParseInput { transaction: None, transaction_file: None, verbose: false, network: &net, keysfile: None };
    let mut buf = Vec::new();
    match parse(&input, &mut buf) {
        Err(ParseError::MissingInput) => {}
        other => panic!("expected MissingInput, got {other:?}"),
    }
}

#[test]
fn test_parse_rejects_invalid_hex() {
    let net = empty_network();
    let input = ParseInput { transaction: Some("zz"), transaction_file: None, verbose: false, network: &net, keysfile: None };
    let mut buf = Vec::new();
    match parse(&input, &mut buf) {
        Err(ParseError::InvalidHex { index: 0, .. }) => {}
        other => panic!("expected InvalidHex, got {other:?}"),
    }
}

#[test]
fn test_parse_emits_expected_text_shape_single_pst() {
    let hex_text = reference_pst_hex();
    let net = empty_network();
    let input = ParseInput { transaction: Some(&hex_text), transaction_file: None, verbose: false, network: &net, keysfile: None };
    let mut buf = Vec::new();
    let count = parse(&input, &mut buf).expect("parse succeeds");
    assert_eq!(count, 1);

    let text = String::from_utf8(buf).expect("output is UTF-8");

    assert!(text.starts_with("Transaction #1 ID: \t"), "header missing; got: {text}");
    let mut lines = text.lines();
    let header = lines.next().unwrap();
    assert!(header.starts_with("Transaction #1 ID: \t"));
    let tx_id_part = &header["Transaction #1 ID: \t".len()..];
    assert_eq!(tx_id_part.len(), 64, "kaspa transaction-id is a 32-byte hash rendered as 64 hex chars: {tx_id_part}");

    assert_eq!(lines.next(), Some(""), "blank line between header and outputs (non-verbose mode skips inputs)");

    let first_output = lines.next().expect("at least one output line");
    assert!(first_output.starts_with("Output 0: \tRecipient: "), "output 0 line shape mismatch: {first_output}");
    assert!(first_output.contains("Amount: "), "output line missing amount: {first_output}");
    assert!(first_output.contains(" Kaspa"), "output line missing Kaspa suffix: {first_output}");

    let mut saw_fee = false;
    for line in lines {
        if line.starts_with("Fee:\t") {
            assert!(line.contains("Sompi ("), "fee line shape: {line}");
            assert!(line.contains(" KAS)"), "fee line shape: {line}");
            saw_fee = true;
        }
    }
    assert!(saw_fee, "fee line must appear: {text}");
}

#[test]
fn test_parse_verbose_emits_input_lines() {
    let hex_text = reference_pst_hex();
    let net = empty_network();
    let input = ParseInput { transaction: Some(&hex_text), transaction_file: None, verbose: true, network: &net, keysfile: None };
    let mut buf = Vec::new();
    parse(&input, &mut buf).expect("parse succeeds");

    let text = String::from_utf8(buf).expect("output is UTF-8");
    let mut saw_input_line = false;
    for line in text.lines() {
        if line.starts_with("Input 0: \t") {
            assert!(line.contains("Outpoint: "), "verbose input line missing outpoint: {line}");
            assert!(line.contains("Amount: "), "verbose input line missing amount: {line}");
            saw_input_line = true;
        }
    }
    assert!(saw_input_line, "verbose mode must emit at least one Input line: {text}");
}

#[test]
fn test_parse_multi_pst_input_is_separator_split() {
    let single = reference_pst_hex();
    let combined = format!("{single}{HEX_TRANSACTIONS_SEPARATOR}{single}");
    let net = empty_network();
    let input = ParseInput { transaction: Some(&combined), transaction_file: None, verbose: false, network: &net, keysfile: None };
    let mut buf = Vec::new();
    let count = parse(&input, &mut buf).expect("parse succeeds");
    assert_eq!(count, 2, "two PSTs separated by '_' parse as two transactions");

    let text = String::from_utf8(buf).expect("output is UTF-8");
    assert!(text.contains("Transaction #1 ID: \t"));
    assert!(text.contains("Transaction #2 ID: \t"));
}

#[test]
fn test_parse_reads_from_file() {
    use std::io::Write;
    let single = reference_pst_hex();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let mut handle = tmp.reopen().unwrap();
    handle.write_all(single.as_bytes()).unwrap();
    handle.flush().unwrap();

    let path = tmp.path().to_string_lossy().into_owned();
    let net = empty_network();
    let input = ParseInput { transaction: None, transaction_file: Some(&path), verbose: false, network: &net, keysfile: None };
    let mut buf = Vec::new();
    let count = parse(&input, &mut buf).expect("parse succeeds");
    assert_eq!(count, 1);
}

#[test]
fn test_parse_emits_mass_and_fee_rate_when_keysfile_supplied() {
    let hex_text = reference_pst_hex();
    let kf = crate::keyfile::read_from_path(fixture("legacy_go_v1_singlekey.json")).expect("keyfile decodes");
    let mut net = empty_network();
    net.testnet = true;
    let input =
        ParseInput { transaction: Some(&hex_text), transaction_file: None, verbose: false, network: &net, keysfile: Some(&kf) };
    let mut buf = Vec::new();
    parse(&input, &mut buf).expect("parse succeeds");

    let text = String::from_utf8(buf).expect("output is UTF-8");
    assert!(text.contains("Mass: "), "Mass line must appear when keysfile is supplied: {text}");
    assert!(text.contains(" grams"), "Mass line must carry 'grams' unit: {text}");
    assert!(text.contains("Fee rate: "), "Fee rate line must appear when keysfile is supplied: {text}");
    assert!(text.contains(" Sompi/Gram"), "Fee rate line must carry 'Sompi/Gram' unit: {text}");
}

#[test]
fn test_parse_omits_mass_and_fee_rate_when_keysfile_absent() {
    let hex_text = reference_pst_hex();
    let net = empty_network();
    let input = ParseInput { transaction: Some(&hex_text), transaction_file: None, verbose: false, network: &net, keysfile: None };
    let mut buf = Vec::new();
    parse(&input, &mut buf).expect("parse succeeds");

    let text = String::from_utf8(buf).expect("output is UTF-8");
    assert!(!text.contains("Mass: "), "Mass line must NOT appear when keysfile is absent: {text}");
    assert!(!text.contains("Fee rate: "), "Fee rate line must NOT appear when keysfile is absent: {text}");
}

#[test]
fn test_resolve_prefix_defaults_to_mainnet() {
    use kaspa_addresses::Prefix;
    let net = empty_network();
    assert_eq!(resolve_prefix(&net), Prefix::Mainnet);

    let mut testnet = empty_network();
    testnet.testnet = true;
    assert_eq!(resolve_prefix(&testnet), Prefix::Testnet);

    let mut simnet = empty_network();
    simnet.simnet = true;
    assert_eq!(resolve_prefix(&simnet), Prefix::Simnet);

    let mut devnet = empty_network();
    devnet.devnet = true;
    assert_eq!(resolve_prefix(&devnet), Prefix::Devnet);
}
