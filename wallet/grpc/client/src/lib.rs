//! Client-side wrapper around the tonic-generated `KaspawalletdClient`.
//!
//! Downstream consumers (the kaspawallet CLI binary, third-party
//! integrators) take a dependency on this crate rather than on
//! `kaspa-wallet-grpc-core` directly, keeping the client-side
//! surface area decoupled from the server-side codegen and
//! convert helpers. The connect builder layers TLS and static
//! API-token authentication on top of the generated client so
//! every consumer wires the same security posture.

use kaspa_wallet_grpc_core::kaspawalletd::kaspawalletd_client::KaspawalletdClient;
use std::fs;
use std::path::{Path, PathBuf};
use tonic::codegen::InterceptedService;
use tonic::metadata::MetadataValue;
use tonic::transport::{Certificate, Channel, ClientTlsConfig, Endpoint, Identity};
use tonic::{Request, Status};

pub use kaspa_wallet_grpc_core::kaspawalletd;

/// Prefix the client attaches to outgoing `authorization`
/// metadata when [`ClientOptions::auth_token`] is configured.
pub const AUTH_TOKEN_PREFIX: &str = "Bearer ";

/// Operator-supplied transport options. None of the fields are
/// required: an empty options struct yields a plain-text client
/// suitable for loopback connections.
#[derive(Debug, Clone, Default)]
pub struct ClientOptions {
    /// Path to a PEM-encoded CA bundle used to validate the
    /// server's TLS certificate. When set, the client connects
    /// over TLS; when unset and `client_cert`/`client_key` are
    /// also unset, the client connects over plain gRPC.
    pub server_ca: Option<PathBuf>,
    /// Domain name to expect in the server's TLS certificate.
    /// Defaults to the endpoint's host when unset.
    pub server_domain: Option<String>,
    /// PEM-encoded client certificate for mutual TLS.
    pub client_cert: Option<PathBuf>,
    /// PEM-encoded private key matching [`Self::client_cert`].
    pub client_key: Option<PathBuf>,
    /// Static API token attached to every outgoing request as
    /// `authorization: Bearer <token>`. Whitespace is trimmed
    /// from the loaded value, matching the daemon's parsing.
    pub auth_token: Option<String>,
}

/// Build a connected [`KaspawalletdClient`] for `endpoint`,
/// applying the TLS and auth-token settings in [`options`].
///
/// The returned service type is parameterised on the auth
/// interceptor closure so calls without auth retain a
/// pass-through interceptor with no per-request metadata cost.
pub async fn connect(
    endpoint: impl Into<String>,
    options: ClientOptions,
) -> Result<KaspawalletdClient<InterceptedService<Channel, AuthInterceptor>>, ClientError> {
    let endpoint_str = endpoint.into();
    let mut builder = Endpoint::from_shared(endpoint_str.clone())
        .map_err(|e| ClientError::Endpoint(format!("invalid endpoint '{endpoint_str}': {e}")))?;

    if let Some(tls) = build_tls(&options)? {
        builder = builder.tls_config(tls).map_err(|e| ClientError::Tls(e.to_string()))?;
    }

    let channel = builder.connect().await.map_err(|e| ClientError::Connect(e.to_string()))?;
    let interceptor = build_auth_interceptor(options.auth_token)?;
    let intercepted = InterceptedService::new(channel, interceptor);
    Ok(KaspawalletdClient::new(intercepted))
}

/// Read a static auth token from `path`, trimming surrounding
/// whitespace so token files written with a trailing newline are
/// accepted verbatim against the server's matching trim.
pub fn load_auth_token(path: &Path) -> Result<String, ClientError> {
    let raw = fs::read_to_string(path).map_err(|e| ClientError::Io(format!("read {}: {e}", path.display())))?;
    let token = raw.trim().to_owned();
    if token.is_empty() {
        return Err(ClientError::EmptyToken(path.display().to_string()));
    }
    Ok(token)
}

fn build_tls(options: &ClientOptions) -> Result<Option<ClientTlsConfig>, ClientError> {
    if options.server_ca.is_none() && options.client_cert.is_none() && options.client_key.is_none() {
        return Ok(None);
    }

    let mut tls = ClientTlsConfig::new();
    if let Some(ca_path) = options.server_ca.as_deref() {
        let ca_pem = fs::read(ca_path).map_err(|e| ClientError::Io(format!("read CA bundle {}: {e}", ca_path.display())))?;
        tls = tls.ca_certificate(Certificate::from_pem(ca_pem));
    }
    if let Some(domain) = options.server_domain.as_deref() {
        tls = tls.domain_name(domain.to_owned());
    }
    match (options.client_cert.as_deref(), options.client_key.as_deref()) {
        (Some(cert_path), Some(key_path)) => {
            let cert_pem =
                fs::read(cert_path).map_err(|e| ClientError::Io(format!("read client cert {}: {e}", cert_path.display())))?;
            let key_pem = fs::read(key_path).map_err(|e| ClientError::Io(format!("read client key {}: {e}", key_path.display())))?;
            tls = tls.identity(Identity::from_pem(cert_pem, key_pem));
        }
        (None, None) => {}
        _ => {
            return Err(ClientError::Tls("client_cert and client_key must be provided together for mutual TLS; got only one".into()));
        }
    }
    Ok(Some(tls))
}

fn build_auth_interceptor(auth_token: Option<String>) -> Result<AuthInterceptor, ClientError> {
    let bearer = match auth_token {
        Some(t) => {
            let header = format!("{AUTH_TOKEN_PREFIX}{t}");
            // Validate up front so a malformed header surfaces as a
            // configuration error rather than per-request status.
            MetadataValue::try_from(header.as_str()).map_err(|e| ClientError::Auth(format!("invalid auth token: {e}")))?;
            Some(header)
        }
        None => None,
    };
    Ok(AuthInterceptor { bearer })
}

/// Tonic-side interceptor that attaches an `authorization`
/// metadata entry to every outgoing request when configured.
/// Pass-through when [`auth_token`](ClientOptions::auth_token)
/// is not set.
#[derive(Debug, Clone)]
pub struct AuthInterceptor {
    bearer: Option<String>,
}

impl tonic::service::Interceptor for AuthInterceptor {
    fn call(&mut self, mut request: Request<()>) -> Result<Request<()>, Status> {
        if let Some(bearer) = self.bearer.as_deref() {
            let value = MetadataValue::try_from(bearer).map_err(|e| Status::internal(format!("attach authorization: {e}")))?;
            request.metadata_mut().insert("authorization", value);
        }
        Ok(request)
    }
}

/// Errors surfaced by [`connect`] and the option-loading helpers.
#[derive(Debug)]
pub enum ClientError {
    Endpoint(String),
    Tls(String),
    Connect(String),
    Io(String),
    EmptyToken(String),
    Auth(String),
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Endpoint(m) => write!(f, "endpoint error: {m}"),
            Self::Tls(m) => write!(f, "TLS configuration error: {m}"),
            Self::Connect(m) => write!(f, "connect error: {m}"),
            Self::Io(m) => write!(f, "io error: {m}"),
            Self::EmptyToken(path) => write!(f, "auth-token file {path} is empty"),
            Self::Auth(m) => write!(f, "auth error: {m}"),
        }
    }
}

impl std::error::Error for ClientError {}
