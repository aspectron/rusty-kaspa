mod args;

use crate::args::Args;
use kaspa_consensus_core::network::NetworkId;
use kaspa_core::{info, warn};
use kaspa_wallet_core::{
    api::WalletApi,
    rpc::{ConnectOptions, ConnectStrategy, Resolver, WrpcEncoding},
    wallet::Wallet,
};
use kaspa_wallet_grpc_core::kaspawalletd::kaspawalletd_server::KaspawalletdServer;
use kaspa_wallet_grpc_server::service::Service;
use std::{error::Error, fs, net::IpAddr, path::Path, str::FromStr, sync::Arc};
use subtle::ConstantTimeEq;
use tokio::sync::oneshot;
use tonic::metadata::MetadataValue;
use tonic::service::interceptor::InterceptedService;
use tonic::transport::{Certificate, Identity, Server, ServerTlsConfig};
use tonic::{Request, Status};

/// Static prefix the daemon expects in the `authorization`
/// metadata when --auth-token is configured. The value after the
/// prefix is the operator-issued bearer token.
const AUTH_TOKEN_PREFIX: &str = "Bearer ";

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    kaspa_core::log::init_logger(None, "");
    let args = Args::parse();

    let tls_config = build_tls_config(&args)?;
    enforce_listen_address_security(&args, tls_config.is_some())?;
    let auth_token = load_auth_token(args.auth_token.as_deref())?;

    let wallet = Arc::new(Wallet::try_new(Wallet::local_store()?, Some(Resolver::default()), None)?);
    wallet.clone().wallet_open(args.password.into(), args.name, false, false).await?;
    info!("Wallet path: {}", wallet.store().location()?);

    if let Some(wrpc_client) = wallet.try_wrpc_client().as_ref() {
        let rpc_address = if let Some(address) = args.rpc_server {
            address
        } else {
            let network_id = NetworkId::from_str(&args.network_id.expect("Specifying network id is needed for PNN"))?;
            warn!("Using PNN may expose your data to third parties. For privacy, use a private, self-hosted node.");
            Resolver::default().get_url(WrpcEncoding::Borsh, network_id).await.map_err(|e| e.to_string())?
        };

        info!("Connecting to {}...", rpc_address);

        let options = ConnectOptions {
            block_async_connect: true,
            strategy: ConnectStrategy::Fallback,
            url: Some(rpc_address),
            ..Default::default()
        };
        wrpc_client.connect(Some(options)).await?;
    }

    let dag_info = wallet.rpc_api().get_block_dag_info().await?;
    wallet.set_network_id(&dag_info.network)?;
    info!("Connected to node on {} with DAA score {}.", dag_info.network, dag_info.virtual_daa_score);

    wallet.start().await?;

    let (shutdown_sender, shutdown_receiver) = oneshot::channel();
    let service = Service::with_notification_pipe_task(wallet.clone(), shutdown_sender);
    service.wallet().accounts_activate(None).await?;
    wallet.autoselect_default_account_if_single().await?;
    info!("Activated account {}, synchronizing...", wallet.account().unwrap().id().short());

    let listen_address = args.listen_address;
    let scheme = if tls_config.is_some() { "https" } else { "http" };
    let auth_status = if auth_token.is_some() { "with static-token auth" } else { "without auth" };
    info!("gRPC server is listening on {}://{} {}.", scheme, listen_address, auth_status);

    let server_handle = tokio::spawn(async move {
        let kaspawalletd = KaspawalletdServer::new(service);
        let intercepted = InterceptedService::new(kaspawalletd, build_auth_interceptor(auth_token));

        let mut builder = Server::builder();
        if let Some(tls) = tls_config {
            builder = builder.tls_config(tls).expect("server TLS configuration is well-formed");
        }
        builder
            .add_service(intercepted)
            .serve_with_shutdown(listen_address, async {
                let _ = shutdown_receiver.await;
                info!("Shutdown initiated, stopping gRPC server...");
            })
            .await
            .unwrap();
    });
    server_handle.await?;

    Ok(())
}

/// Build the `ServerTlsConfig` from the operator-provided
/// certificate, private key, and optional client-CA paths. Returns
/// `Ok(None)` when --tls-cert and --tls-key are both absent (plain
/// gRPC posture); errors when only one of the two is provided
/// because half-configured TLS is a footgun, not an opt-out.
fn build_tls_config(args: &Args) -> Result<Option<ServerTlsConfig>, Box<dyn Error>> {
    match (args.tls_cert.as_deref(), args.tls_key.as_deref()) {
        (Some(cert_path), Some(key_path)) => {
            let cert_pem = fs::read(cert_path).map_err(|e| format!("read --tls-cert {}: {e}", cert_path.display()))?;
            let key_pem = fs::read(key_path).map_err(|e| format!("read --tls-key {}: {e}", key_path.display()))?;
            let identity = Identity::from_pem(cert_pem, key_pem);
            let mut tls = ServerTlsConfig::new().identity(identity);
            if let Some(ca_path) = args.client_ca.as_deref() {
                let ca_pem = fs::read(ca_path).map_err(|e| format!("read --client-ca {}: {e}", ca_path.display()))?;
                tls = tls.client_ca_root(Certificate::from_pem(ca_pem));
            }
            Ok(Some(tls))
        }
        (None, None) => {
            if args.client_ca.is_some() {
                return Err(
                    "--client-ca requires --tls-cert and --tls-key; cannot verify client certificates without server TLS".into()
                );
            }
            Ok(None)
        }
        (Some(_), None) => Err("--tls-cert without --tls-key is invalid; provide both or neither".into()),
        (None, Some(_)) => Err("--tls-key without --tls-cert is invalid; provide both or neither".into()),
    }
}

/// Refuse to bind a non-loopback listen address without TLS unless
/// the operator passes --insecure. The default posture is "127.0.0.1
/// only"; any wider exposure is a deliberate operator choice that
/// requires either a real TLS identity or the explicit insecure
/// opt-in.
fn enforce_listen_address_security(args: &Args, tls_enabled: bool) -> Result<(), Box<dyn Error>> {
    let ip = args.listen_address.ip();
    if is_loopback(ip) || tls_enabled || args.insecure {
        return Ok(());
    }
    Err(format!(
        "refusing to listen on non-loopback address {ip} without TLS. \
         Provide --tls-cert / --tls-key, or pass --insecure if you really want plain gRPC on a public address."
    )
    .into())
}

fn is_loopback(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_loopback(),
        IpAddr::V6(v6) => v6.is_loopback(),
    }
}

/// Read the static API token from --auth-token, trimming trailing
/// whitespace so a token file written with a trailing newline still
/// matches an exact bearer header.
fn load_auth_token(path: Option<&Path>) -> Result<Option<String>, Box<dyn Error>> {
    let Some(path) = path else { return Ok(None) };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = fs::metadata(path).map_err(|e| format!("stat --auth-token {}: {e}", path.display()))?;
        let mode = metadata.permissions().mode();
        if mode & 0o077 != 0 {
            return Err(format!(
                "--auth-token file {} is group/world readable (mode {:#o}); restrict to owner-only (e.g. chmod 600)",
                path.display(),
                mode & 0o777,
            )
            .into());
        }
    }
    let raw = fs::read_to_string(path).map_err(|e| format!("read --auth-token {}: {e}", path.display()))?;
    let token = raw.trim().to_owned();
    if token.is_empty() {
        return Err(format!("--auth-token file {} is empty", path.display()).into());
    }
    Ok(Some(token))
}

/// Build the gRPC request interceptor. When `auth_token` is set,
/// every inbound request must carry an `authorization: Bearer
/// <token>` metadata entry whose suffix matches the configured
/// token verbatim. When `auth_token` is `None`, the interceptor is
/// a pass-through.
fn build_auth_interceptor(auth_token: Option<String>) -> impl FnMut(Request<()>) -> Result<Request<()>, Status> + Clone {
    let expected_header = auth_token.map(|t| format!("{AUTH_TOKEN_PREFIX}{t}"));
    move |request: Request<()>| -> Result<Request<()>, Status> {
        let Some(expected) = expected_header.as_ref() else {
            return Ok(request);
        };
        let presented = request.metadata().get("authorization").and_then(|v| v.to_str().ok()).unwrap_or("");
        if MetadataValue::try_from(expected.as_str()).is_ok()
            && bool::from(presented.as_bytes().ct_eq(expected.as_bytes()))
        {
            Ok(request)
        } else {
            Err(Status::unauthenticated("missing or invalid authorization token"))
        }
    }
}
