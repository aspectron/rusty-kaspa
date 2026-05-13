//! Subcommand-registry and flag-parity tests.

use clap::CommandFactory;

use super::args::Cli;

/// The full 17-subcommand surface registered on the binary's
/// `clap` `Command`.
const EXPECTED_SUBCOMMANDS: &[&str] = &[
    "create",
    "dump-unencrypted-data",
    "start-daemon",
    "balance",
    "send",
    "create-unsigned-transaction",
    "sign",
    "broadcast",
    "parse",
    "show-addresses",
    "new-address",
    "version",
    "sweep",
    "broadcast-replacement",
    "bump-fee",
    "bump-fee-unsigned",
    "get-daemon-version",
];

#[test]
fn test_subcommand_registry_matches_expected_set() {
    let mut cmd = Cli::command();
    let mut got: Vec<String> = cmd.get_subcommands_mut().map(|s| s.get_name().to_string()).collect();
    got.sort();
    let mut want: Vec<String> = EXPECTED_SUBCOMMANDS.iter().map(|s| (*s).to_string()).collect();
    want.sort();
    assert_eq!(got, want, "registered subcommands deviate from the expected surface");
}

#[test]
fn test_subcommand_count_is_seventeen() {
    let cmd = Cli::command();
    let count = cmd.get_subcommands().count();
    assert_eq!(count, 17, "wallet binary exposes exactly 17 subcommands");
}

#[test]
fn test_create_flag_set() {
    let cmd = Cli::command();
    let sub = cmd.find_subcommand("create").expect("create subcommand registered");
    let longs: Vec<&str> = sub.get_arguments().filter_map(|a| a.get_long()).collect();
    let expected = [
        "keys-file",
        "password",
        "yes",
        "min-signatures",
        "num-private-keys",
        "num-public-keys",
        "ecdsa",
        "import",
        "testnet",
        "simnet",
        "devnet",
        "override-dag-params-file",
    ];
    for ex in &expected {
        assert!(longs.contains(ex), "create is missing flag --{ex}: {longs:?}");
    }
}

#[test]
fn test_send_required_to_address() {
    let cmd = Cli::command();
    let sub = cmd.find_subcommand("send").expect("send subcommand registered");
    let to_address = sub.get_arguments().find(|a| a.get_long() == Some("to-address")).expect("send has --to-address");
    assert!(to_address.is_required_set(), "send --to-address must be required");
}

#[test]
fn test_create_unsigned_required_to_address() {
    let cmd = Cli::command();
    let sub = cmd.find_subcommand("create-unsigned-transaction").expect("create-unsigned-transaction registered");
    let to_address = sub.get_arguments().find(|a| a.get_long() == Some("to-address")).expect("subcommand has --to-address");
    assert!(to_address.is_required_set(), "create-unsigned-transaction --to-address must be required");
}

#[test]
fn test_balance_daemon_address_default_is_loopback() {
    let cmd = Cli::command();
    let sub = cmd.find_subcommand("balance").expect("balance subcommand registered");
    let arg = sub.get_arguments().find(|a| a.get_long() == Some("daemonaddress")).expect("balance has --daemonaddress");
    let defaults: Vec<&clap::builder::OsStr> = arg.get_default_values().iter().collect();
    let default_os: &std::ffi::OsStr = defaults.first().expect("default present").as_ref();
    let default_str = default_os.to_str().unwrap_or("");
    assert_eq!(default_str, "localhost:8082", "daemon address default must be loopback-only");
}

#[test]
fn test_start_daemon_listen_default_is_loopback() {
    let cmd = Cli::command();
    let sub = cmd.find_subcommand("start-daemon").expect("start-daemon subcommand registered");
    let arg = sub.get_arguments().find(|a| a.get_long() == Some("listen")).expect("start-daemon has --listen");
    let defaults: Vec<&clap::builder::OsStr> = arg.get_default_values().iter().collect();
    let default_os: &std::ffi::OsStr = defaults.first().expect("default present").as_ref();
    let default_str = default_os.to_str().unwrap_or("");
    assert_eq!(default_str, super::args::DEFAULT_START_DAEMON_LISTEN, "--listen default must be the loopback IP form");
    assert_eq!(
        super::args::DEFAULT_START_DAEMON_LISTEN,
        "127.0.0.1:8082",
        "loopback default must remain a literal IP for SocketAddr parsing"
    );
}

#[test]
fn test_start_daemon_password_is_required() {
    use clap::Parser;

    let err = Cli::try_parse_from(["kaspawallet", "start-daemon"]).expect_err("--password is mandatory");
    let s = format!("{err}");
    assert!(s.contains("password"), "missing-required error must name --password: {s}");
}

#[test]
fn test_start_daemon_insecure_flag_round_trips() {
    use clap::Parser;

    let cli = Cli::try_parse_from(["kaspawallet", "start-daemon", "--password", "/tmp/pw", "--insecure"])
        .expect("start-daemon parses with --insecure");
    if let super::args::Subcommand::StartDaemon(args) = cli.command {
        assert!(args.insecure, "--insecure must round-trip into StartDaemonArgs::insecure");
        assert_eq!(args.password.to_string_lossy(), "/tmp/pw");
    } else {
        panic!("expected StartDaemon subcommand");
    }
}

#[test]
fn test_start_daemon_tls_and_auth_flags_round_trip() {
    use clap::Parser;

    let cli = Cli::try_parse_from([
        "kaspawallet",
        "start-daemon",
        "--password",
        "/tmp/pw",
        "--listen",
        "0.0.0.0:9090",
        "--tls-cert",
        "/tmp/cert.pem",
        "--tls-key",
        "/tmp/key.pem",
        "--client-ca",
        "/tmp/ca.pem",
        "--auth-token",
        "/tmp/token",
        "--name",
        "alpha",
        "--rpc-server",
        "127.0.0.1:16110",
    ])
    .expect("start-daemon parses with the full flag set");
    if let super::args::Subcommand::StartDaemon(args) = cli.command {
        assert_eq!(args.listen.to_string(), "0.0.0.0:9090");
        assert_eq!(args.tls_cert.as_ref().unwrap().to_string_lossy(), "/tmp/cert.pem");
        assert_eq!(args.tls_key.as_ref().unwrap().to_string_lossy(), "/tmp/key.pem");
        assert_eq!(args.client_ca.as_ref().unwrap().to_string_lossy(), "/tmp/ca.pem");
        assert_eq!(args.auth_token.as_ref().unwrap().to_string_lossy(), "/tmp/token");
        assert_eq!(args.name.as_deref(), Some("alpha"));
        assert_eq!(args.rpc_server.as_deref(), Some("127.0.0.1:16110"));
    } else {
        panic!("expected StartDaemon subcommand");
    }
}

#[test]
fn test_no_wallet_backend_flag_anywhere() {
    let cmd = Cli::command();
    for sub in cmd.get_subcommands() {
        let has_flag = sub.get_arguments().any(|a| a.get_long() == Some("wallet-backend"));
        assert!(!has_flag, "integrated binary must NOT carry --wallet-backend on subcommand {}", sub.get_name());
    }
}

#[test]
fn test_short_flag_no_conflicts_within_each_subcommand() {
    let cmd = Cli::command();
    for sub in cmd.get_subcommands() {
        let mut seen = std::collections::HashSet::new();
        for arg in sub.get_arguments() {
            if let Some(short) = arg.get_short()
                && !seen.insert(short)
            {
                panic!("subcommand {} reuses short flag -{short}", sub.get_name());
            }
        }
    }
}

#[test]
fn test_subcommand_parsing_smoke() {
    use clap::Parser;

    let cli = Cli::try_parse_from(["kaspawallet", "version"]).expect("version parses");
    assert!(matches!(cli.command, super::args::Subcommand::Version(_)));

    let cli = Cli::try_parse_from(["kaspawallet", "send", "--to-address", "kaspa:qabcd", "--send-amount", "1.5"])
        .expect("send parses with mandatory to-address");
    if let super::args::Subcommand::Send(args) = cli.command {
        assert_eq!(args.to_address, "kaspa:qabcd");
        assert_eq!(args.send_amount.as_deref(), Some("1.5"));
    } else {
        panic!("expected Send subcommand");
    }

    let err = Cli::try_parse_from(["kaspawallet", "send", "--send-amount", "1"]).expect_err("missing required");
    let s = format!("{err}");
    assert!(s.contains("to-address"), "missing-required error must name --to-address: {s}");
}

#[test]
fn test_rbf_subcommands_parse_with_expected_flags() {
    use clap::Parser;

    let cli = Cli::try_parse_from([
        "kaspawallet",
        "bump-fee",
        "--txid",
        "deadbeef",
        "--password",
        "pw",
        "--max-fee-rate",
        "12.5",
        "--show-serialized",
    ])
    .expect("bump-fee parses");
    if let super::args::Subcommand::BumpFee(args) = cli.command {
        assert_eq!(args.txid.as_deref(), Some("deadbeef"));
        assert_eq!(args.password.as_deref(), Some("pw"));
        assert_eq!(args.max_fee_rate, Some(12.5));
        assert!(args.show_serialized);
    } else {
        panic!("expected BumpFee subcommand");
    }

    let cli = Cli::try_parse_from(["kaspawallet", "bump-fee-unsigned", "--txid", "feedface", "--fee-rate", "3.0"])
        .expect("bump-fee-unsigned parses");
    if let super::args::Subcommand::BumpFeeUnsigned(args) = cli.command {
        assert_eq!(args.txid.as_deref(), Some("feedface"));
        assert_eq!(args.fee_rate, Some(3.0));
    } else {
        panic!("expected BumpFeeUnsigned subcommand");
    }

    let cli =
        Cli::try_parse_from(["kaspawallet", "broadcast-replacement", "--transaction", "00ff"]).expect("broadcast-replacement parses");
    if let super::args::Subcommand::BroadcastReplacement(args) = cli.command {
        assert_eq!(args.transaction.as_deref(), Some("00ff"));
    } else {
        panic!("expected BroadcastReplacement subcommand");
    }

    let cli = Cli::try_parse_from(["kaspawallet", "get-daemon-version"]).expect("get-daemon-version parses with defaults");
    if let super::args::Subcommand::GetDaemonVersion(args) = cli.command {
        assert_eq!(args.daemon_address, super::args::DEFAULT_LISTEN);
    } else {
        panic!("expected GetDaemonVersion subcommand");
    }
}

#[test]
fn test_top_level_network_flag_combines_with_subcommand() {
    use super::network::NetworkFlags;
    use clap::Parser;

    let cli = Cli::try_parse_from(["kaspawallet", "--testnet", "balance"]).expect("top-level --testnet parses");
    let mut merged = match cli.command {
        super::args::Subcommand::Balance(args) => args.network,
        _ => panic!("expected Balance subcommand"),
    };
    let top: NetworkFlags = cli.network;
    merged.combine(&top);
    assert!(merged.testnet, "top-level --testnet must merge into subcommand's NetworkFlags");
    assert_eq!(merged.network_name(), "kaspa-testnet-10");
}

#[test]
fn test_network_flag_names_match_registered_longs() {
    use super::network::NETWORK_FLAG_NAMES;

    let cmd = Cli::command();
    let sub = cmd.find_subcommand("balance").expect("balance subcommand registered");
    let longs: Vec<&str> = sub.get_arguments().filter_map(|a| a.get_long()).collect();
    for name in NETWORK_FLAG_NAMES {
        assert!(longs.contains(name), "balance subcommand missing network flag --{name}: {longs:?}");
    }
}

#[test]
fn test_address_prefix_resolves_each_network() {
    use super::network::NetworkFlags;

    let mainnet = NetworkFlags::default();
    assert!(matches!(mainnet.address_prefix(), kaspa_addresses::Prefix::Mainnet));

    let testnet = NetworkFlags { testnet: true, ..Default::default() };
    assert!(matches!(testnet.address_prefix(), kaspa_addresses::Prefix::Testnet));

    let simnet = NetworkFlags { simnet: true, ..Default::default() };
    assert!(matches!(simnet.address_prefix(), kaspa_addresses::Prefix::Simnet));

    let devnet = NetworkFlags { devnet: true, ..Default::default() };
    assert!(matches!(devnet.address_prefix(), kaspa_addresses::Prefix::Devnet));
}

#[test]
fn test_format_kas_zero_is_nineteen_spaces() {
    let s = super::dispatch::format_kas(0);
    assert_eq!(s.len(), 19);
    assert!(s.chars().all(|c| c == ' '), "zero amount must format as 19 spaces, got {s:?}");
}

#[test]
fn test_format_kas_one_kas() {
    let s = super::dispatch::format_kas(super::dispatch::SOMPI_PER_KASPA);
    assert_eq!(s.len(), 19);
    assert_eq!(s, "         1.00000000");
}

#[test]
fn test_format_kas_one_sompi() {
    let s = super::dispatch::format_kas(1);
    assert_eq!(s.len(), 19);
    assert_eq!(s, "         0.00000001");
}

#[test]
fn test_format_kas_large_value_keeps_eight_decimals() {
    let s = super::dispatch::format_kas(123_456_789_012_345);
    assert!(s.ends_with(".89012345"), "8-decimal tail must survive large amounts, got {s:?}");
}
