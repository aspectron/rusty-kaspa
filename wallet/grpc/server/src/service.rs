use fee_policy::FeePolicy;
use futures_util::{select, FutureExt};
use kaspa_consensus_core::constants::SOMPI_PER_KASPA;
use kaspa_wallet_core::{
    api::WalletApi,
    events::Events,
    prelude::{AccountDescriptor, Address},
    wallet::Wallet,
};
use kaspa_wallet_grpc_core::kaspawalletd;
use kaspa_wallet_grpc_core::kaspawalletd::fee_policy;
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;
use tonic::Status;

pub struct Service {
    wallet: Arc<Wallet>,
    shutdown_sender: Arc<Mutex<Option<oneshot::Sender<()>>>>,
    // TODO: Extend the partially serialized transaction or transaction structure with a boolean field 'ecdsa'
    ecdsa: bool,
}

impl Service {
    pub fn with_notification_pipe_task(wallet: Arc<Wallet>, shutdown_sender: oneshot::Sender<()>, ecdsa: bool) -> Self {
        let channel = wallet.multiplexer().channel();

        tokio::spawn({
            let wallet = wallet.clone();

            async move {
                loop {
                    select! {
                        msg = channel.receiver.recv().fuse() => {
                            if let Ok(msg) = msg {
                                match *msg {
                                    Events::SyncState { sync_state } => {
                                        if sync_state.is_synced() {
                                            if let Err(err) = wallet.clone().wallet_reload(false).await {
                                                panic!("Wallet reloading failed: {}", err)
                                            }
                                        }
                                    },
                                    Events::Balance { balance: _new_balance, .. } => {
                                        // TBD: index balance per address for call
                                    },
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
        });

        Service { wallet, shutdown_sender: Arc::new(Mutex::new(Some(shutdown_sender))), ecdsa }
    }

    // TODO: maybe create custom error type
    pub async fn calculate_fee_limits(&self, fee_policy: Option<kaspawalletd::FeePolicy>) -> Result<(f64, u64), Status> {
        let fee_policy = match fee_policy {
            Some(fee_policy) => fee_policy.fee_policy,
            None => None,
        };
        self._calculate_fee_limits(fee_policy).await
    }

    // TODO: rename
    pub async fn _calculate_fee_limits(&self, fee_policy: Option<FeePolicy>) -> Result<(f64, u64), Status> {
        const MIN_FEE_RATE: f64 = 1.0;
        let fees: (f64, u64) = if let Some(policy) = fee_policy {
            match policy {
                FeePolicy::MaxFeeRate(max_fee_rate) => {
                    if max_fee_rate < MIN_FEE_RATE {
                        return Err(Status::invalid_argument(format!(
                            "requested max fee rate {} is too low, minimum fee rate is {}",
                            max_fee_rate, MIN_FEE_RATE
                        )));
                    };
                    let estimate = self.wallet.rpc_api().get_fee_estimate().await.unwrap();
                    let fee_rate = max_fee_rate.min(estimate.normal_buckets[0].feerate);
                    (fee_rate, u64::MAX)
                }
                FeePolicy::ExactFeeRate(exact_fee_rate) => {
                    if exact_fee_rate < MIN_FEE_RATE {
                        return Err(Status::invalid_argument(format!(
                            "requested fee rate {} is too low, minimum fee rate is {}",
                            exact_fee_rate, MIN_FEE_RATE
                        )));
                    }
                    (exact_fee_rate, u64::MAX)
                }
                FeePolicy::MaxFee(max_fee) => {
                    let estimate = self.wallet.rpc_api().get_fee_estimate().await.unwrap();
                    (estimate.normal_buckets[0].feerate, max_fee)
                }
            }
        } else {
            let estimate = self.wallet.rpc_api().get_fee_estimate().await.unwrap();
            (estimate.normal_buckets[0].feerate, SOMPI_PER_KASPA)
        };
        Ok(fees)
    }

    pub fn receive_addresses(&self) -> Vec<Address> {
        // TODO: move into WalletApi
        let manager = self.wallet.account().unwrap().as_derivation_capable().unwrap().derivation().receive_address_manager();
        manager.get_range_with_args(0..manager.index(), false).unwrap()
    }

    pub fn wallet(&self) -> Arc<Wallet> {
        self.wallet.clone()
    }

    pub fn descriptor(&self) -> AccountDescriptor {
        self.wallet.account().unwrap().descriptor().unwrap()
    }

    pub fn initiate_shutdown(&self) {
        let mut sender = self.shutdown_sender.lock().unwrap();
        if let Some(shutdown_sender) = sender.take() {
            let _ = shutdown_sender.send(());
        }
    }

    /// Returns whether the service should use ECDSA signatures instead of Schnorr signatures.
    /// This flag is used when processing transactions to determine the appropriate signature scheme.
    /// Currently set via command-line arguments, but this is temporary - the signature scheme
    /// should be determined per transaction by extending the partially serialized transaction
    /// or transaction structure with this field.
    pub fn use_ecdsa(&self) -> bool {
        self.ecdsa
    }
}
