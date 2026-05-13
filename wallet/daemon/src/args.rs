use clap::{Arg, ArgAction, Command};
use kaspa_core::kaspad_env::version;
use std::net::SocketAddr;
use std::path::PathBuf;

pub struct Args {
    pub password: String,
    pub name: Option<String>,
    pub rpc_server: Option<String>,
    pub network_id: Option<String>,
    pub listen: SocketAddr,
    pub tls_cert: Option<PathBuf>,
    pub tls_key: Option<PathBuf>,
    pub client_ca: Option<PathBuf>,
    pub auth_token: Option<PathBuf>,
    pub insecure: bool,
}

impl Args {
    pub fn parse() -> Self {
        let matches = cli().get_matches();

        Args {
            password: matches.get_one::<String>("password").cloned().expect("Password argument is missing."),
            name: matches.get_one::<String>("name").cloned(),
            rpc_server: matches.get_one::<String>("rpc-server").cloned(),
            network_id: matches.get_one::<String>("network-id").cloned(),
            listen: matches
                .get_one::<SocketAddr>("listen")
                .cloned()
                .unwrap_or_else(|| "127.0.0.1:8082".parse().unwrap()),
            tls_cert: matches.get_one::<PathBuf>("tls-cert").cloned(),
            tls_key: matches.get_one::<PathBuf>("tls-key").cloned(),
            client_ca: matches.get_one::<PathBuf>("client-ca").cloned(),
            auth_token: matches.get_one::<PathBuf>("auth-token").cloned(),
            insecure: matches.get_flag("insecure"),
        }
    }
}

pub fn cli() -> Command {
    Command::new("kaspawalletd")
        .about(format!("{} (kaspawalletd) v{}", env!("CARGO_PKG_DESCRIPTION"), version()))
        .version(env!("CARGO_PKG_VERSION"))
        .arg(Arg::new("password").long("password").short('p').value_name("password").help("Path of password file").required(true))
        .arg(
            Arg::new("name")
                .long("name")
                .short('n')
                .value_name("name")
                .value_parser(clap::value_parser!(String))
                .help("Name of wallet"),
        )
        .arg(
            Arg::new("rpc-server")
                .long("rpc-server")
                .short('s')
                .value_name("rpc-server")
                .value_parser(clap::value_parser!(String))
                .help("Private RPC server URL"),
        )
        .arg(
            Arg::new("network-id")
                .long("network-id")
                .value_name("network-id")
                .value_parser(clap::value_parser!(String))
                .help("Network id to be connected via PNN."),
        )
        .arg(
            Arg::new("listen")
                .long("listen")
                .short('l')
                .value_name("listen")
                .value_parser(clap::value_parser!(String))
                .help("gRPC listening address with port."),
        )
        .arg(
            Arg::new("tls-cert")
                .long("tls-cert")
                .value_name("path")
                .value_parser(clap::value_parser!(PathBuf))
                .help("Path to a PEM-encoded TLS certificate. Required (together with --tls-key) to serve over TLS."),
        )
        .arg(
            Arg::new("tls-key")
                .long("tls-key")
                .value_name("path")
                .value_parser(clap::value_parser!(PathBuf))
                .help("Path to a PEM-encoded TLS private key matching --tls-cert."),
        )
        .arg(Arg::new("client-ca").long("client-ca").value_name("path").value_parser(clap::value_parser!(PathBuf)).help(
            "Path to a PEM-encoded CA certificate. When set, the server requires mutually authenticated TLS \
                       and verifies client certificates against this CA.",
        ))
        .arg(Arg::new("auth-token").long("auth-token").value_name("path").value_parser(clap::value_parser!(PathBuf)).help(
            "Path to a file containing a static API token. When set, the server rejects requests whose \
                       `authorization` metadata does not match `Bearer <token>`.",
        ))
        .arg(Arg::new("insecure").long("insecure").action(ArgAction::SetTrue).help(
            "Allow a non-loopback --listen without TLS. Off by default; required to expose the \
                       daemon to a remote host over plain gRPC.",
        ))
}
