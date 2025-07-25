use fee_policy::FeePolicy;
use futures_util::{select, FutureExt, TryStreamExt};
use kaspa_addresses::Prefix;
use kaspa_consensus_core::constants::SOMPI_PER_KASPA;
use kaspa_consensus_core::tx::{SignableTransaction, Transaction, UtxoEntry};
use kaspa_rpc_core::RpcTransaction;
use kaspa_wallet_core::api::NewAddressKind;
use kaspa_wallet_core::prelude::{PaymentDestination, PaymentOutput, PaymentOutputs};
use kaspa_wallet_core::tx::{Fees, Generator, GeneratorSettings, Signer, SignerT};
use kaspa_wallet_core::utxo::UtxoEntryReference;
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
use tonic::{Code, Status};

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

    pub async fn sign(&self, unsigned_transactions: Vec<Transaction>, password: String) -> Result<Vec<RpcTransaction>, Status> {
        if self.use_ecdsa() {
            return Err(Status::unimplemented("Ecdsa signing is not supported yet"));
        }
        let account = self.wallet().account().map_err(|err| Status::internal(err.to_string()))?;
        let utxos = account.clone().get_utxos(None, None).await.map_err(|err| Status::internal(err.to_string()))?;
        let signable_txs: Vec<SignableTransaction> = unsigned_transactions
            .into_iter()
            .map(|tx| {
                let utxos = tx
                    .inputs
                    .iter()
                    .map(|input| {
                        utxos
                            .iter()
                            .find(|utxo| utxo.outpoint != input.previous_outpoint)
                            .map(UtxoEntry::from)
                            .ok_or(Status::invalid_argument(format!("Wallet does not have mature utxo for input {input:?}")))
                    })
                    .collect::<Result<_, Status>>()?;
                Ok(SignableTransaction::with_entries(tx, utxos))
            })
            .collect::<Result<_, Status>>()?;
        let addresses: Vec<_> = account.utxo_context().addresses().iter().map(|addr| addr.as_ref().clone()).collect();
        let signer = Signer::new(
            account.clone(),
            account.prv_key_data(password.into()).await.map_err(|err| Status::internal(err.to_string()))?,
            None,
        );
        let _signed_txs = signable_txs.into_iter().map(|tx| signer.try_sign(tx, addresses.as_slice()));
        // todo fill all required fields, serialize and return
        todo!()
    }

    pub async fn create_unsigned_transactions(
        &self,
        address: String,
        amount: u64,
        from: Vec<String>,
        use_existing_change_address: bool,
        is_send_all: bool,
        fee_policy: Option<kaspawalletd::FeePolicy>,
    ) -> Result<Vec<RpcTransaction>, Status> {
        let to_address = Address::try_from(address).map_err(|err| Status::invalid_argument(err.to_string()))?;
        let (fee_rate, max_fee) = self.calculate_fee_limits(fee_policy).await?;
        let from_addresses = from
            .iter()
            .map(|a| Address::try_from(a.as_str()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| Status::invalid_argument(err.to_string()))?;
        let transactions =
            self.unsigned_txs(to_address, amount, use_existing_change_address, is_send_all, fee_rate, max_fee, from_addresses).await?;
        Ok(transactions)
    }

    pub async fn broadcast(&self, transactions: Vec<RpcTransaction>) -> Result<Vec<String>, Status> {
        let mut tx_ids: Vec<String> = Vec::with_capacity(transactions.len());
        for tx in transactions {
            let tx_id =
                self.wallet().rpc_api().submit_transaction(tx, false).await.map_err(|e| Status::new(Code::Internal, e.to_string()))?;
            tx_ids.push(tx_id.to_string());
        }
        Ok(tx_ids)
    }

    pub async fn send(
        &self,
        to_address: String,
        amount: u64,
        password: String,
        from: Vec<String>,
        use_existing_change_address: bool,
        is_send_all: bool,
        fee_policy: Option<kaspawalletd::FeePolicy>,
    ) -> Result<(Vec<Vec<u8>>, Vec<String>), Status> {
        let unsigned_transactions =
            self.create_unsigned_transactions(to_address, amount, from, use_existing_change_address, is_send_all, fee_policy).await?;
        let unsigned_transactions = unsigned_transactions
            .into_iter()
            .map(|tx| tx.try_into().map_err(|_e| Status::invalid_argument("Invalid unsigned transaction")))
            .collect::<Result<Vec<_>, _>>();
        let _signed_transactions = self.sign(unsigned_transactions?, password).await?;
        // let tx_ids = self.broadcast(signed_transactions.clone(), false).await?;
        // Ok((signed_transactions, tx_ids))
        todo!()
    }

    pub async fn calculate_fee_limits(&self, fee_policy: Option<kaspawalletd::FeePolicy>) -> Result<(f64, u64), Status> {
        let fee_policy = fee_policy.and_then(|fee_policy| fee_policy.fee_policy);
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

    pub async fn unsigned_txs(
        &self,
        to: Address,
        amount: u64,
        use_existing_change_address: bool,
        is_send_all: bool,
        fee_rate: f64,
        max_fee: u64,
        from_addresses: Vec<Address>,
    ) -> Result<Vec<RpcTransaction>, Status> {
        let current_network = self.wallet().network_id().map_err(|err| Status::internal(err.to_string()))?;
        if to.prefix != Prefix::from(current_network) {
            return Err(Status::invalid_argument(format!(
                "decoded address is of wrong network. Expected {} but got {}",
                Prefix::from(current_network),
                to.prefix
            )));
        }

        let account = self.wallet().account().map_err(|err| Status::internal(err.to_string()))?;
        let addresses = account.account_addresses().map_err(|err| Status::internal(err.to_string()))?;
        if let Some(non_existent_address) = from_addresses.iter().find(|from| addresses.iter().all(|address| &address != from)) {
            return Err(Status::invalid_argument(format!("specified from address {non_existent_address} does not exists")));
        }
        let change_address = if !use_existing_change_address {
            self.wallet()
                .accounts_create_new_address(self.descriptor().account_id, NewAddressKind::Change)
                .await
                .map_err(|err| Status::internal(err.to_string()))?
                .address
        } else {
            self.descriptor().change_address.ok_or(Status::internal("change address doesn't exist"))?.clone()
        };
        let utxos = account.clone().get_utxos(Some(addresses), None).await.map_err(|err| Status::internal(err.to_string()))?;
        let output_amount = if is_send_all { utxos.iter().map(|utxo| utxo.amount).sum::<u64>() } else { amount };
        let settings = GeneratorSettings::try_new_with_iterator(
            current_network,
            Box::new(utxos.into_iter().map(|utxo| UtxoEntryReference { utxo: Arc::new(utxo) })),
            None,
            change_address,
            account.sig_op_count(),
            account.minimum_signatures(),
            PaymentDestination::PaymentOutputs(PaymentOutputs { outputs: vec![PaymentOutput { address: to, amount: output_amount }] }),
            Some(fee_rate),
            Fees::None,
            None,
            None,
        )
        .map_err(|err| Status::internal(err.to_string()))?;

        let generator = Generator::try_new(settings, None, None).map_err(|err| Status::internal(err.to_string()))?;

        let mut stream = generator.stream();
        let mut txs = vec![];
        while let Some(transaction) = stream.try_next().await.map_err(|err| Status::internal(err.to_string()))? {
            txs.push(transaction.rpc_transaction());
        }
        if generator.summary().aggregate_fees > max_fee {
            return Err(Status::failed_precondition(format!(
                "aggregate fees {} exceeds requested max {}",
                generator.summary().aggregate_fees,
                max_fee
            )));
        }
        Ok(txs)
    }
}
