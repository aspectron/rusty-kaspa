use clap::{Arg, Command};
use kaspa_core::kaspad_env::version;
use std::io;
use std::net::SocketAddr;
use std::path::PathBuf;

pub struct Args {
    pub password: String,
    pub name: Option<String>,
    pub rpc_server: Option<String>,
    pub network_id: Option<String>,
    pub listen_address: SocketAddr,
    pub ecdsa: bool,
    pub location: Option<PathBuf>,
}

impl Args {
    pub fn parse() -> Result<Self, Box<dyn std::error::Error>> {
        let matches = cli().get_matches();
        let key_file = matches.get_one::<PathBuf>("keys-file").cloned();
        let (name, location) = parse_keys_file_arg(key_file)?;
        Ok(Args {
            password: matches.get_one::<String>("password").cloned().expect("Password argument is missing."),
            name,
            rpc_server: matches.get_one::<String>("rpc-server").cloned(),
            network_id: matches.get_one::<String>("network-id").cloned(),
            listen_address: matches
                .get_one::<SocketAddr>("listen-address")
                .cloned()
                .unwrap_or_else(|| "127.0.0.1:8082".parse().unwrap()),
            ecdsa: matches.get_one::<bool>("ecdsa").cloned().unwrap_or(false),
            location,
        })
    }
}

pub fn cli() -> Command {
    Command::new("kaspawalletd")
        .about(format!("{} (kaspawalletd) v{}", env!("CARGO_PKG_DESCRIPTION"), version()))
        .version(env!("CARGO_PKG_VERSION"))
        .arg(Arg::new("password").long("password").short('p').value_name("password").help("Path of password file").required(true))
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
            Arg::new("listen-address")
                .long("listen-address")
                .short('l')
                .value_name("listen-address")
                .value_parser(clap::value_parser!(String))
                .help("gRPC listening address with port."),
        )
        .arg(
            Arg::new("ecdsa")
                .long("ecdsa")
                .value_name("ecdsa")
                .value_parser(clap::value_parser!(bool))
                .help("Use ecdsa for transactions broadcast"),
        )
        .arg(
            Arg::new("keys-file")
                .long("keys-file")
                .short('f')
                .value_name("keys-file")
                .value_parser(clap::value_parser!(PathBuf))
                .help("Keys file location"),
        )
}

fn parse_keys_file_arg(keys_file: Option<PathBuf>) -> Result<(Option<String>, Option<PathBuf>), Box<dyn std::error::Error>> {
    if let Some(keys_file) = keys_file {
        if keys_file.is_dir() {
            Ok((None, Some(keys_file)))
        } else {
            let name = keys_file
                .file_name()
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Invalid wallet file path"))?
                .to_str()
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Wallet file path is not valid UTF-8"))?
                .to_owned();
            Ok((Some(name), keys_file.parent().map(|p| p.to_owned())))
        }
    } else {
        Ok((None, None))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_parse_keys_file() {
        // arrange
        let tmp_keys_file = NamedTempFile::new().unwrap();

        // act
        let (name, location) = parse_keys_file_arg(Some(tmp_keys_file.path().to_path_buf())).unwrap();

        // assert
        assert_eq!(name, Some(tmp_keys_file.path().file_name().unwrap().to_str().unwrap().to_owned()));
        assert_eq!(location, Some(tmp_keys_file.path().parent().unwrap().to_path_buf()));
    }
}
