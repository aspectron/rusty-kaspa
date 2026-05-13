mod args;

use crate::args::Args;
use kaspa_wallet_daemon::{ServeOptions, run};
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    kaspa_core::log::init_logger(None, "");
    let args = Args::parse();
    let opts = ServeOptions {
        password: args.password,
        name: args.name,
        rpc_server: args.rpc_server,
        network_id: args.network_id,
        listen: args.listen,
        tls_cert: args.tls_cert,
        tls_key: args.tls_key,
        client_ca: args.client_ca,
        auth_token: args.auth_token,
        insecure: args.insecure,
    };
    run(opts).await
}
