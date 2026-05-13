//! `create` subcommand.
//!
//! Generates fresh BIP-39 mnemonics (default) or imports
//! operator-supplied mnemonics from stdin (`--import`); derives the
//! cosigner-level extended public key for each; encrypts every
//! mnemonic with the operator's password under the v1 keyfile
//! format (Argon2id m=64 MiB t=1 threads=8; XChaCha20-Poly1305
//! AEAD), and writes the resulting [`KeysFile`] to disk.
//!
//! For multisig wallets where the local operator holds fewer
//! seeds than the total cosigner count
//! (`--num-public-keys > --num-private-keys`), the remaining
//! cosigner extended public keys are read from stdin and stored
//! in the keyfile's `publicKeys` array alongside the
//! locally-derived ones; the keyfile's `cosignerIndex` field is
//! the minimum sorted-order position of any locally-held xpub
//! within the full set.
//!
//! Default keyfile path is platform-aware
//! (`<app-dir>/<network>/keys.json`); operators override with
//! `--keys-file`. Refuses to overwrite an existing keyfile unless
//! `--yes` is passed.
//!
//! Operator-visible output is line-by-line equivalent to the
//! reference binary's `create` output: per-mnemonic xpub blocks,
//! the secret-key vs public-address disclaimer, per-cosigner xpub
//! prompts when applicable, and the final
//! `Wrote the keys into <path>` terminator.

use std::fs;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::str::FromStr;

use kaspa_bip32::{ExtendedPublicKey, Language, Mnemonic, WordCount};

use crate::cli::args::CreateArgs;
use crate::cli::dispatch::{fail, master_xpub_from_mnemonic, merge_network, require_password_with_confirmation, xpub_prefix};
use crate::cli::network::NetworkFlags;
use crate::keyfile::{self, EncryptedMnemonic, KeysFile, LATEST_VERSION};
use crate::keysource::default_keys_file;

/// `numThreads` value written into v1 keyfiles produced by
/// `create`. Matches the encrypt-time default the keyfile codec
/// already pins.
const NUM_THREADS_ON_WRITE: u8 = 8;

pub fn run_create(args: CreateArgs, top: &NetworkFlags) -> ExitCode {
    let stdin = io::stdin();
    let mut reader = stdin.lock();
    run_create_with_reader(args, top, &mut reader)
}

fn run_create_with_reader<R: BufRead>(args: CreateArgs, top: &NetworkFlags, reader: &mut R) -> ExitCode {
    if args.num_private_keys == 0 {
        return fail("'create': --num-private-keys must be at least 1");
    }
    if args.num_public_keys < args.num_private_keys {
        return fail("'create': --num-public-keys must be at least --num-private-keys");
    }
    let password = match require_password_with_confirmation(&args.password, &args.password_file, "create") {
        Ok(p) => p,
        Err(e) => return e,
    };
    let network = merge_network(top, &args.network);
    let prefix = xpub_prefix(&network);
    let is_multisig = args.num_public_keys > 1;

    let resolved_path = match resolve_keyfile_path(args.keys_file.as_deref(), &network) {
        Ok(p) => p,
        Err(e) => return e,
    };
    if resolved_path.exists() && !args.yes {
        return fail(format!("keyfile '{}' already exists; pass --yes to overwrite", resolved_path.display()));
    }

    let (encrypted, local_xpubs) =
        match build_local_records(args.import, args.num_private_keys, is_multisig, prefix, password.as_bytes(), reader) {
            Ok(pair) => pair,
            Err(e) => return e,
        };

    for (i, xpub) in local_xpubs.iter().enumerate() {
        println!("Extended public key of mnemonic #{}:\n{xpub}\n", i + 1);
    }

    println!(
        "Notice the above is neither a secret key to your wallet (use \"kaspawallet dump-unencrypted-data\" to see a secret seed phrase) \
nor a wallet public address (use \"kaspawallet new-address\" to create and see one)\n"
    );

    let mut all_xpubs: Vec<String> = Vec::with_capacity(args.num_public_keys as usize);
    all_xpubs.extend_from_slice(&local_xpubs);
    if args.num_public_keys > args.num_private_keys {
        match read_cosigner_xpubs(reader, args.num_private_keys, args.num_public_keys) {
            Ok(xs) => all_xpubs.extend(xs),
            Err(e) => return e,
        }
    }

    let cosigner_index = match minimum_cosigner_index(&local_xpubs, &all_xpubs) {
        Ok(idx) => idx,
        Err(msg) => return fail(msg),
    };

    let kf = KeysFile {
        version: LATEST_VERSION,
        num_threads: NUM_THREADS_ON_WRITE,
        encrypted_mnemonics: encrypted,
        extended_public_keys: all_xpubs,
        minimum_signatures: args.min_signatures,
        cosigner_index,
        last_used_external_index: 0,
        last_used_internal_index: 0,
        ecdsa: args.ecdsa,
    };

    if let Some(parent) = resolved_path.parent()
        && let Err(e) = fs::create_dir_all(parent)
    {
        return fail(format!("failed to create keyfile parent '{}': {e}", parent.display()));
    }
    if let Err(e) = keyfile::save_to_path(&kf, &resolved_path) {
        return fail(format!("save keyfile '{}': {e}", resolved_path.display()));
    }
    println!("Wrote the keys into {}", resolved_path.display());
    ExitCode::SUCCESS
}

fn resolve_keyfile_path(override_path: Option<&str>, network: &NetworkFlags) -> Result<PathBuf, ExitCode> {
    match override_path {
        Some(p) => Ok(PathBuf::from(p)),
        None => default_keys_file(network.network_name()).map_err(|e| fail(format!("default keyfile path resolution failed: {e}"))),
    }
}

fn build_local_records<R: BufRead>(
    import: bool,
    count: u32,
    is_multisig: bool,
    prefix: kaspa_bip32::Prefix,
    password: &[u8],
    reader: &mut R,
) -> Result<(Vec<EncryptedMnemonic>, Vec<String>), ExitCode> {
    if import {
        import_mnemonic_records(reader, count, is_multisig, prefix, password)
    } else {
        mint_mnemonic_records(count, is_multisig, prefix, password)
    }
}

fn mint_mnemonic_records(
    count: u32,
    is_multisig: bool,
    prefix: kaspa_bip32::Prefix,
    password: &[u8],
) -> Result<(Vec<EncryptedMnemonic>, Vec<String>), ExitCode> {
    let n = count as usize;
    let mut encrypted: Vec<EncryptedMnemonic> = Vec::with_capacity(n);
    let mut xpubs: Vec<String> = Vec::with_capacity(n);
    for _ in 0..n {
        let mnemonic =
            Mnemonic::random(WordCount::Words24, Language::English).map_err(|e| fail(format!("mnemonic generation: {e}")))?;
        let phrase = mnemonic.phrase().to_owned();
        let xpub = master_xpub_from_mnemonic(&phrase, is_multisig, prefix).map_err(|e| fail(format!("xpub derivation: {e}")))?;
        let record = keyfile::encrypt_mnemonic(&phrase, password).map_err(|e| fail(format!("mnemonic encryption: {e}")))?;
        xpubs.push(xpub);
        encrypted.push(record);
    }
    Ok((encrypted, xpubs))
}

fn import_mnemonic_records<R: BufRead>(
    reader: &mut R,
    count: u32,
    is_multisig: bool,
    prefix: kaspa_bip32::Prefix,
    password: &[u8],
) -> Result<(Vec<EncryptedMnemonic>, Vec<String>), ExitCode> {
    let n = count as usize;
    let mut encrypted: Vec<EncryptedMnemonic> = Vec::with_capacity(n);
    let mut xpubs: Vec<String> = Vec::with_capacity(n);
    for i in 0..n {
        println!("Enter mnemonic #{} here:", i + 1);
        let _ = io::stdout().flush();
        let phrase = match read_trimmed_line(reader) {
            Ok(s) if !s.is_empty() => s,
            Ok(_) => return Err(fail("mnemonic is invalid")),
            Err(e) => return Err(fail(format!("read mnemonic: {e}"))),
        };
        Mnemonic::new(&phrase, Language::English).map_err(|_| fail("mnemonic is invalid"))?;
        let xpub = master_xpub_from_mnemonic(&phrase, is_multisig, prefix).map_err(|e| fail(format!("xpub derivation: {e}")))?;
        let record = keyfile::encrypt_mnemonic(&phrase, password).map_err(|e| fail(format!("mnemonic encryption: {e}")))?;
        xpubs.push(xpub);
        encrypted.push(record);
    }
    Ok((encrypted, xpubs))
}

fn read_cosigner_xpubs<R: BufRead>(reader: &mut R, first_index: u32, total: u32) -> Result<Vec<String>, ExitCode> {
    let mut out: Vec<String> = Vec::with_capacity((total - first_index) as usize);
    for i in first_index..total {
        println!("Enter public key #{} here:", i + 1);
        let _ = io::stdout().flush();
        let xpub = match read_trimmed_line(reader) {
            Ok(s) if !s.is_empty() => s,
            Ok(_) => return Err(fail("cosigner extended public key is empty")),
            Err(e) => return Err(fail(format!("read cosigner extended public key: {e}"))),
        };
        ExtendedPublicKey::<secp256k1::PublicKey>::from_str(&xpub)
            .map_err(|e| fail(format!("{xpub} is invalid extended public key: {e}")))?;
        out.push(xpub);
        println!();
    }
    Ok(out)
}

fn read_trimmed_line<R: BufRead>(reader: &mut R) -> io::Result<String> {
    let mut line = String::new();
    if reader.read_line(&mut line)? == 0 {
        return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "end of input"));
    }
    Ok(line.trim().to_owned())
}

/// Position of the lowest-sorted local xpub within the full
/// cosigner set, lexicographic on the encoded xpub string. The
/// keyfile reader uses this index to address the operator's
/// private-key slot when signing.
fn minimum_cosigner_index(local_xpubs: &[String], all_xpubs: &[String]) -> Result<u32, String> {
    if local_xpubs.is_empty() {
        return Ok(0);
    }
    let mut sorted: Vec<&str> = all_xpubs.iter().map(String::as_str).collect();
    sorted.sort();
    let mut min: u32 = u32::MAX;
    for x in local_xpubs {
        let idx = sorted
            .iter()
            .position(|s| *s == x.as_str())
            .ok_or_else(|| format!("local extended public key not found in keyfile set: {x}"))?;
        let idx_u32 = u32::try_from(idx).map_err(|_| format!("cosigner index overflow: {idx}"))?;
        if idx_u32 < min {
            min = idx_u32;
        }
    }
    Ok(min)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::args::{Cli, Subcommand};
    use clap::Parser;
    use std::io::Cursor;
    use tempfile::tempdir;

    /// Known-valid 24-word BIP-39 mnemonic (Trezor test vector,
    /// public). Re-used across multiple tests to assert
    /// byte-identical xpub derivation between the import path
    /// and a direct `kaspa-bip32` reference.
    const VALID_24_WORDS: &str = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon \
         abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art";

    fn parse(args: &[&str]) -> Cli {
        let mut full = vec!["kaspawallet"];
        full.extend(args);
        Cli::parse_from(full)
    }

    fn create_args(c: &Cli) -> &CreateArgs {
        match &c.command {
            Subcommand::Create(a) => a,
            other => panic!("expected Create subcommand, got {other:?}"),
        }
    }

    fn create_subcmd(cli: Cli) -> (CreateArgs, crate::cli::network::NetworkFlags) {
        let net = cli.network.clone();
        let cmd = match cli.command {
            Subcommand::Create(a) => a,
            other => panic!("expected Create, got {other:?}"),
        };
        (cmd, net)
    }

    #[test]
    fn test_create_defaults() {
        let cli = parse(&["create", "--password", "pw"]);
        let args = create_args(&cli);
        assert_eq!(args.password.as_deref(), Some("pw"));
        assert_eq!(args.min_signatures, 1);
        assert_eq!(args.num_private_keys, 1);
        assert_eq!(args.num_public_keys, 1);
        assert!(!args.ecdsa);
        assert!(!args.import);
        assert!(!args.yes);
        assert!(args.keys_file.is_none());
    }

    #[test]
    fn test_create_ecdsa_flag() {
        let cli = parse(&["create", "--password", "pw", "--ecdsa"]);
        assert!(create_args(&cli).ecdsa);
    }

    #[test]
    fn test_create_keys_file_override() {
        let cli = parse(&["create", "--password", "pw", "--keys-file", "/tmp/custom.json"]);
        assert_eq!(create_args(&cli).keys_file.as_deref(), Some("/tmp/custom.json"));
    }

    #[test]
    fn test_create_yes_round_trips() {
        let cli = parse(&["create", "--password", "pw", "--yes"]);
        assert!(create_args(&cli).yes);
    }

    #[test]
    fn test_create_multisig_thresholds() {
        let cli = parse(&["create", "--password", "pw", "--num-private-keys", "3", "--num-public-keys", "3", "--min-signatures", "2"]);
        let a = create_args(&cli);
        assert_eq!(a.num_private_keys, 3);
        assert_eq!(a.num_public_keys, 3);
        assert_eq!(a.min_signatures, 2);
    }

    #[test]
    fn test_run_create_writes_decodable_keyfile() {
        let dir = tempdir().expect("tempdir");
        let kf_path = dir.path().join("nested").join("keys.json");
        let kf_str = kf_path.to_string_lossy().to_string();
        let cli = parse(&["create", "--password", "correct horse battery staple", "--keys-file", &kf_str]);
        let args = match cli.command {
            Subcommand::Create(a) => a,
            other => panic!("expected Create, got {other:?}"),
        };
        let exit = run_create(args, &cli.network);
        // Exit codes are opaque; we assert via the round-trip
        // below: if `run_create` failed it returned exit-1 and
        // no keyfile is on disk.
        assert!(kf_path.exists(), "expected keyfile at {} (run_create exit was {exit:?})", kf_path.display());

        let loaded = keyfile::read_from_path(&kf_path).expect("read produced keyfile");
        assert_eq!(loaded.version, LATEST_VERSION);
        assert_eq!(loaded.num_threads, NUM_THREADS_ON_WRITE);
        assert_eq!(loaded.encrypted_mnemonics.len(), 1);
        assert_eq!(loaded.extended_public_keys.len(), 1);
        assert_eq!(loaded.minimum_signatures, 1);
        assert!(!loaded.ecdsa);
        assert!(
            loaded.extended_public_keys[0].starts_with("kpub"),
            "mainnet xpub prefix expected, got {}",
            loaded.extended_public_keys[0]
        );
    }

    #[test]
    fn test_run_create_refuses_existing_without_yes() {
        let dir = tempdir().expect("tempdir");
        let kf_path = dir.path().join("keys.json");
        fs::write(&kf_path, b"placeholder").expect("placeholder write");
        let kf_str = kf_path.to_string_lossy().to_string();
        let cli = parse(&["create", "--password", "pw", "--keys-file", &kf_str]);
        let args = match cli.command {
            Subcommand::Create(a) => a,
            other => panic!("expected Create, got {other:?}"),
        };
        let _exit = run_create(args, &cli.network);
        // Did not overwrite: file still has placeholder bytes.
        let bytes = fs::read(&kf_path).expect("read placeholder");
        assert_eq!(bytes, b"placeholder", "create must refuse to overwrite without --yes");
    }

    #[test]
    fn test_run_create_overwrites_with_yes() {
        let dir = tempdir().expect("tempdir");
        let kf_path = dir.path().join("keys.json");
        fs::write(&kf_path, b"placeholder").expect("placeholder write");
        let kf_str = kf_path.to_string_lossy().to_string();
        let cli = parse(&["create", "--password", "pw", "--keys-file", &kf_str, "--yes"]);
        let args = match cli.command {
            Subcommand::Create(a) => a,
            other => panic!("expected Create, got {other:?}"),
        };
        let _exit = run_create(args, &cli.network);
        let loaded = keyfile::read_from_path(&kf_path).expect("read produced keyfile");
        assert_eq!(loaded.version, LATEST_VERSION);
    }

    #[test]
    fn test_run_create_rejects_mismatched_key_counts() {
        let dir = tempdir().expect("tempdir");
        let kf_path = dir.path().join("keys.json");
        let kf_str = kf_path.to_string_lossy().to_string();
        let cli = parse(&["create", "--password", "pw", "--keys-file", &kf_str, "--num-private-keys", "2", "--num-public-keys", "1"]);
        let args = match cli.command {
            Subcommand::Create(a) => a,
            other => panic!("expected Create, got {other:?}"),
        };
        let _exit = run_create(args, &cli.network);
        assert!(!kf_path.exists(), "rejected create must not write a keyfile");
    }

    #[test]
    fn test_run_create_rejects_missing_password() {
        let dir = tempdir().expect("tempdir");
        let kf_path = dir.path().join("keys.json");
        let kf_str = kf_path.to_string_lossy().to_string();
        let cli = parse(&["create", "--keys-file", &kf_str]);
        let args = match cli.command {
            Subcommand::Create(a) => a,
            other => panic!("expected Create, got {other:?}"),
        };
        let _exit = run_create(args, &cli.network);
        assert!(!kf_path.exists(), "rejected create must not write a keyfile");
    }

    #[test]
    fn test_run_create_testnet_uses_ktub_prefix() {
        let dir = tempdir().expect("tempdir");
        let kf_path = dir.path().join("keys.json");
        let kf_str = kf_path.to_string_lossy().to_string();
        let cli = parse(&["--testnet", "create", "--password", "pw", "--keys-file", &kf_str]);
        let args = match cli.command {
            Subcommand::Create(a) => a,
            other => panic!("expected Create, got {other:?}"),
        };
        let _exit = run_create(args, &cli.network);
        let loaded = keyfile::read_from_path(&kf_path).expect("read produced keyfile");
        assert!(
            loaded.extended_public_keys[0].starts_with("ktub"),
            "testnet xpub prefix expected, got {}",
            loaded.extended_public_keys[0]
        );
    }

    #[test]
    fn test_run_create_import_rejects_bad_bip39_checksum() {
        // Swap the last word from `art` to a different valid
        // BIP-39 word; the resulting 24-word phrase no longer
        // satisfies the checksum constraint and must reject.
        let bad_mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon \
             abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon ability";
        let dir = tempdir().expect("tempdir");
        let kf_path = dir.path().join("keys.json");
        let kf_str = kf_path.to_string_lossy().to_string();
        let cli = parse(&["create", "--import", "--password", "pw", "--keys-file", &kf_str]);
        let (args, network) = create_subcmd(cli);
        let stdin_bytes = format!("{bad_mnemonic}\n");
        let mut reader = Cursor::new(stdin_bytes.into_bytes());
        let _exit = run_create_with_reader(args, &network, &mut reader);
        assert!(!kf_path.exists(), "bad BIP-39 checksum must NOT produce a keyfile");
    }

    #[test]
    fn test_run_create_import_byte_identical_to_reference() {
        let dir = tempdir().expect("tempdir");
        let kf_path = dir.path().join("keys.json");
        let kf_str = kf_path.to_string_lossy().to_string();
        let cli = parse(&["create", "--import", "--password", "pw", "--keys-file", &kf_str]);
        let (args, network) = create_subcmd(cli);
        let stdin_bytes = format!("{VALID_24_WORDS}\n");
        let mut reader = Cursor::new(stdin_bytes.into_bytes());
        let _exit = run_create_with_reader(args, &network, &mut reader);
        assert!(kf_path.exists(), "valid mnemonic must produce a keyfile");

        let loaded = keyfile::read_from_path(&kf_path).expect("read produced keyfile");
        assert_eq!(loaded.extended_public_keys.len(), 1);

        // Reference: derive the cosigner xpub directly via the
        // shared `master_xpub_from_mnemonic` helper. The keyfile's
        // publicKey[0] MUST be byte-identical to the reference for
        // the restore-from-mnemonic invariant to hold.
        let reference =
            master_xpub_from_mnemonic(VALID_24_WORDS, /*is_multisig=*/ false, xpub_prefix(&network)).expect("reference xpub");
        assert_eq!(
            loaded.extended_public_keys[0], reference,
            "import-derived publicKey[0] must be byte-identical to the kaspa-bip32 reference"
        );
        assert!(!loaded.ecdsa, "Schnorr-default import must store ecdsa=false");
    }

    #[test]
    fn test_run_create_import_ecdsa_records_curve_flag() {
        // Per Go-wallet reference, the `--ecdsa` flag affects the
        // keyfile's `ecdsa` selector and downstream leaf-address +
        // signing primitives; the cosigner-level xpub at
        // BIP-44 m/44'/111111'/0' does NOT depend on the curve
        // selector. This test asserts both: import records
        // `ecdsa=true`, and publicKey[0] matches the Schnorr-import
        // value bit-for-bit because the master-pubkey derivation is
        // curve-independent.
        let dir = tempdir().expect("tempdir");
        let kf_schnorr_path = dir.path().join("schnorr.json");
        let kf_ecdsa_path = dir.path().join("ecdsa.json");

        let cli_s = parse(&["create", "--import", "--password", "pw", "--keys-file", &kf_schnorr_path.to_string_lossy()]);
        let (args_s, network_s) = create_subcmd(cli_s);
        let mut reader_s = Cursor::new(format!("{VALID_24_WORDS}\n").into_bytes());
        let _ = run_create_with_reader(args_s, &network_s, &mut reader_s);
        let schnorr_kf = keyfile::read_from_path(&kf_schnorr_path).expect("schnorr keyfile");

        let cli_e = parse(&["create", "--import", "--ecdsa", "--password", "pw", "--keys-file", &kf_ecdsa_path.to_string_lossy()]);
        let (args_e, network_e) = create_subcmd(cli_e);
        let mut reader_e = Cursor::new(format!("{VALID_24_WORDS}\n").into_bytes());
        let _ = run_create_with_reader(args_e, &network_e, &mut reader_e);
        let ecdsa_kf = keyfile::read_from_path(&kf_ecdsa_path).expect("ecdsa keyfile");

        assert!(!schnorr_kf.ecdsa, "schnorr-default keyfile must have ecdsa=false");
        assert!(ecdsa_kf.ecdsa, "--ecdsa keyfile must have ecdsa=true");
        assert_eq!(
            schnorr_kf.extended_public_keys[0], ecdsa_kf.extended_public_keys[0],
            "cosigner-level publicKey[0] is curve-independent and must match between Schnorr and ECDSA imports of the same mnemonic"
        );
    }

    #[test]
    fn test_run_create_cosigner_split_assembles_keyfile() {
        // 1-of-2 wallet: operator holds 1 mnemonic locally and
        // contributes 1 cosigner xpub via stdin. The keyfile must
        // contain 1 encrypted mnemonic, 2 publicKeys entries, and a
        // cosignerIndex that points to the local xpub's position in
        // the lexicographic sort of all xpubs.
        let dir = tempdir().expect("tempdir");
        let kf_path = dir.path().join("ms.json");
        let kf_str = kf_path.to_string_lossy().to_string();

        // Derive a deterministic external cosigner xpub from a
        // separate known mnemonic so the test does not depend on a
        // hardcoded string from the network's address space.
        let external_mnemonic = "legal winner thank year wave sausage worth useful legal winner thank year wave sausage worth useful legal winner thank year wave sausage worth title";
        let cli = parse(&[
            "create",
            "--import",
            "--password",
            "pw",
            "--keys-file",
            &kf_str,
            "--num-private-keys",
            "1",
            "--num-public-keys",
            "2",
            "--min-signatures",
            "1",
        ]);
        let (args, network) = create_subcmd(cli);
        let external_xpub =
            master_xpub_from_mnemonic(external_mnemonic, /*is_multisig=*/ true, xpub_prefix(&network)).expect("external xpub");
        let stdin_bytes = format!("{VALID_24_WORDS}\n{external_xpub}\n");
        let mut reader = Cursor::new(stdin_bytes.into_bytes());
        let _ = run_create_with_reader(args, &network, &mut reader);
        assert!(kf_path.exists(), "cosigner-split create must produce a keyfile");

        let loaded = keyfile::read_from_path(&kf_path).expect("read produced keyfile");
        assert_eq!(loaded.encrypted_mnemonics.len(), 1, "exactly one encrypted local mnemonic");
        assert_eq!(loaded.extended_public_keys.len(), 2, "two cosigner xpubs total");

        let local_xpub = master_xpub_from_mnemonic(VALID_24_WORDS, true, xpub_prefix(&network)).expect("local xpub");
        assert!(loaded.extended_public_keys.contains(&local_xpub), "local xpub must be present in keyfile");
        assert!(loaded.extended_public_keys.contains(&external_xpub), "external xpub must be present in keyfile");

        // Expected cosignerIndex: position of local_xpub in the
        // lexicographic sort of {local_xpub, external_xpub}.
        let mut sorted = [local_xpub.as_str(), external_xpub.as_str()];
        sorted.sort();
        let expected_idx = sorted.iter().position(|s| *s == local_xpub.as_str()).expect("local in sort") as u32;
        assert_eq!(loaded.cosigner_index, expected_idx, "cosignerIndex must point to local xpub's sorted position");
    }

    #[test]
    fn test_run_create_cosigner_split_rejects_invalid_xpub() {
        let dir = tempdir().expect("tempdir");
        let kf_path = dir.path().join("ms.json");
        let kf_str = kf_path.to_string_lossy().to_string();
        let cli = parse(&[
            "create",
            "--import",
            "--password",
            "pw",
            "--keys-file",
            &kf_str,
            "--num-private-keys",
            "1",
            "--num-public-keys",
            "2",
        ]);
        let (args, network) = create_subcmd(cli);
        let stdin_bytes = format!("{VALID_24_WORDS}\nnot-a-real-xpub\n");
        let mut reader = Cursor::new(stdin_bytes.into_bytes());
        let _ = run_create_with_reader(args, &network, &mut reader);
        assert!(!kf_path.exists(), "invalid cosigner xpub must NOT produce a keyfile");
    }

    #[test]
    fn test_minimum_cosigner_index_picks_min_sorted_position() {
        // Two-local + two-external scenario. Expected index is the
        // minimum sorted position over the two local entries.
        let local = vec!["zzz".to_owned(), "aaa".to_owned()];
        let all = vec!["aaa".to_owned(), "mmm".to_owned(), "zzz".to_owned(), "bbb".to_owned()];
        // Sorted: [aaa(0), bbb(1), mmm(2), zzz(3)]; locals at 0 and 3; min = 0.
        assert_eq!(minimum_cosigner_index(&local, &all).expect("computed"), 0);
    }

    #[test]
    fn test_minimum_cosigner_index_single_local_in_middle() {
        let local = vec!["mmm".to_owned()];
        let all = vec!["zzz".to_owned(), "aaa".to_owned(), "mmm".to_owned()];
        // Sorted: [aaa(0), mmm(1), zzz(2)]; local at 1.
        assert_eq!(minimum_cosigner_index(&local, &all).expect("computed"), 1);
    }

    // --- Per-argument coverage (every CreateArgs field exercised
    // against an observable keyfile property).

    #[test]
    fn test_run_create_ecdsa_flag_propagates_to_keyfile() {
        let dir = tempdir().expect("tempdir");
        let kf_path = dir.path().join("ecdsa.json");
        let kf_str = kf_path.to_string_lossy().to_string();
        let cli = parse(&["create", "--ecdsa", "--password", "pw", "--keys-file", &kf_str]);
        let args = match cli.command {
            Subcommand::Create(a) => a,
            other => panic!("expected Create, got {other:?}"),
        };
        let _ = run_create(args, &cli.network);
        let loaded = keyfile::read_from_path(&kf_path).expect("read keyfile");
        assert!(loaded.ecdsa, "--ecdsa must set keyfile.ecdsa=true");
    }

    #[test]
    fn test_run_create_password_file_succeeds() {
        let dir = tempdir().expect("tempdir");
        let pw_path = dir.path().join("pw.txt");
        fs::write(&pw_path, b"file-sourced-secret\n").expect("write password file");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&pw_path, fs::Permissions::from_mode(0o600)).expect("chmod 0600");
        }
        let kf_path = dir.path().join("keys.json");
        let cli = parse(&["create", "--password-file", &pw_path.to_string_lossy(), "--keys-file", &kf_path.to_string_lossy()]);
        let args = match cli.command {
            Subcommand::Create(a) => a,
            other => panic!("expected Create, got {other:?}"),
        };
        let _ = run_create(args, &cli.network);
        assert!(kf_path.exists(), "--password-file must drive the keyfile-encrypt path without an interactive prompt");
        let loaded = keyfile::read_from_path(&kf_path).expect("read keyfile");
        assert_eq!(loaded.encrypted_mnemonics.len(), 1);
    }

    #[test]
    fn test_run_create_min_signatures_propagates() {
        let dir = tempdir().expect("tempdir");
        let kf_path = dir.path().join("keys.json");
        let kf_str = kf_path.to_string_lossy().to_string();
        let cli = parse(&[
            "create",
            "--password",
            "pw",
            "--keys-file",
            &kf_str,
            "--num-private-keys",
            "3",
            "--num-public-keys",
            "3",
            "--min-signatures",
            "2",
        ]);
        let args = match cli.command {
            Subcommand::Create(a) => a,
            other => panic!("expected Create, got {other:?}"),
        };
        let _ = run_create(args, &cli.network);
        let loaded = keyfile::read_from_path(&kf_path).expect("read keyfile");
        assert_eq!(loaded.minimum_signatures, 2, "--min-signatures must propagate to keyfile.minimumSignatures");
    }

    #[test]
    fn test_run_create_local_multisig_writes_n_keys() {
        // --num-private-keys=3 --num-public-keys=3 (all local):
        // 3 encrypted mnemonics + 3 distinct multisig-derived
        // cosigner xpubs in the keyfile.
        let dir = tempdir().expect("tempdir");
        let kf_path = dir.path().join("keys.json");
        let kf_str = kf_path.to_string_lossy().to_string();
        let cli = parse(&[
            "create",
            "--password",
            "pw",
            "--keys-file",
            &kf_str,
            "--num-private-keys",
            "3",
            "--num-public-keys",
            "3",
            "--min-signatures",
            "2",
        ]);
        let args = match cli.command {
            Subcommand::Create(a) => a,
            other => panic!("expected Create, got {other:?}"),
        };
        let _ = run_create(args, &cli.network);
        let loaded = keyfile::read_from_path(&kf_path).expect("read keyfile");
        assert_eq!(loaded.encrypted_mnemonics.len(), 3);
        assert_eq!(loaded.extended_public_keys.len(), 3);
        // All-local: no external xpubs, so cosignerIndex is the
        // sorted position of the lowest local xpub, which is 0.
        assert_eq!(loaded.cosigner_index, 0);
    }

    #[test]
    fn test_run_create_devnet_uses_ktub_prefix() {
        let dir = tempdir().expect("tempdir");
        let kf_path = dir.path().join("keys.json");
        let kf_str = kf_path.to_string_lossy().to_string();
        let cli = parse(&["--devnet", "create", "--password", "pw", "--keys-file", &kf_str]);
        let args = match cli.command {
            Subcommand::Create(a) => a,
            other => panic!("expected Create, got {other:?}"),
        };
        let _ = run_create(args, &cli.network);
        let loaded = keyfile::read_from_path(&kf_path).expect("read keyfile");
        assert!(
            loaded.extended_public_keys[0].starts_with("ktub"),
            "devnet xpub prefix expected, got {}",
            loaded.extended_public_keys[0]
        );
    }

    #[test]
    fn test_run_create_simnet_uses_ktub_prefix() {
        let dir = tempdir().expect("tempdir");
        let kf_path = dir.path().join("keys.json");
        let kf_str = kf_path.to_string_lossy().to_string();
        let cli = parse(&["--simnet", "create", "--password", "pw", "--keys-file", &kf_str]);
        let args = match cli.command {
            Subcommand::Create(a) => a,
            other => panic!("expected Create, got {other:?}"),
        };
        let _ = run_create(args, &cli.network);
        let loaded = keyfile::read_from_path(&kf_path).expect("read keyfile");
        assert!(
            loaded.extended_public_keys[0].starts_with("ktub"),
            "simnet xpub prefix expected, got {}",
            loaded.extended_public_keys[0]
        );
    }

    /// When `--keys-file` is omitted, the default-path resolver
    /// fires and the keyfile lands at the platform-specific default
    /// location. POSIX-only to keep the path-suffix assertion
    /// portable; non-POSIX coverage is in `keysource::default_path`.
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    #[test]
    fn test_run_create_default_keyfile_path_used_when_none() {
        let dir = tempdir().expect("tempdir");
        // SAFETY: test-only mutation. nextest spawns each test in
        // its own process, so the HOME swap does not race other
        // tests that read HOME.
        unsafe {
            std::env::set_var("HOME", dir.path());
        }
        let cli = parse(&["create", "--password", "pw"]);
        let args = match cli.command {
            Subcommand::Create(a) => a,
            other => panic!("expected Create, got {other:?}"),
        };
        let _ = run_create(args, &cli.network);
        let expected = default_keys_file(cli.network.network_name()).expect("default-path resolver");
        assert!(expected.exists(), "default keyfile must exist at {}", expected.display());
        assert!(expected.starts_with(dir.path()), "default keyfile must live under the HOME override; got {}", expected.display());
    }

    #[test]
    fn test_run_create_import_default_keys_match_minted_keys_property() {
        // Import-path and mint-path produce a keyfile of the same
        // shape: same num_threads, version, ecdsa default. Only the
        // mnemonic source differs.
        let dir = tempdir().expect("tempdir");
        let imp_path = dir.path().join("import.json");
        let cli_i = parse(&["create", "--import", "--password", "pw", "--keys-file", &imp_path.to_string_lossy()]);
        let (args_i, network_i) = create_subcmd(cli_i);
        let mut reader_i = Cursor::new(format!("{VALID_24_WORDS}\n").into_bytes());
        let _ = run_create_with_reader(args_i, &network_i, &mut reader_i);
        let imp = keyfile::read_from_path(&imp_path).expect("import keyfile");

        let mint_path = dir.path().join("mint.json");
        let cli_m = parse(&["create", "--password", "pw", "--keys-file", &mint_path.to_string_lossy()]);
        let args_m = match cli_m.command {
            Subcommand::Create(a) => a,
            other => panic!("expected Create, got {other:?}"),
        };
        let _ = run_create(args_m, &cli_m.network);
        let mint = keyfile::read_from_path(&mint_path).expect("mint keyfile");

        assert_eq!(imp.version, mint.version);
        assert_eq!(imp.num_threads, mint.num_threads);
        assert_eq!(imp.ecdsa, mint.ecdsa);
        assert_eq!(imp.minimum_signatures, mint.minimum_signatures);
        assert_eq!(imp.encrypted_mnemonics.len(), mint.encrypted_mnemonics.len());
        assert_eq!(imp.extended_public_keys.len(), mint.extended_public_keys.len());
    }
}
