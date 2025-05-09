pub mod service;

use kaspa_rpc_core::RpcTransaction;
use kaspa_wallet_core::api::WalletApi;
use kaspa_wallet_core::{
    api::{AccountsGetUtxosRequest, AccountsSendRequest, NewAddressKind},
    prelude::Address,
    tx::{Fees, PaymentDestination, PaymentOutputs},
};
use kaspa_wallet_grpc_core::kaspawalletd::{
    fee_policy::FeePolicy, kaspawalletd_server::Kaspawalletd, BroadcastRequest, BroadcastResponse, BumpFeeRequest, BumpFeeResponse,
    CreateUnsignedTransactionsRequest, CreateUnsignedTransactionsResponse, GetBalanceRequest, GetBalanceResponse,
    GetExternalSpendableUtxOsRequest, GetExternalSpendableUtxOsResponse, GetVersionRequest, GetVersionResponse, NewAddressRequest,
    NewAddressResponse, SendRequest, SendResponse, ShowAddressesRequest, ShowAddressesResponse, ShutdownRequest, ShutdownResponse,
    SignRequest, SignResponse,
};
use kaspa_wallet_grpc_core::protoserialization::TransactionMessage;
use prost::Message;
use service::Service;
use tonic::{Request, Response, Status};

#[tonic::async_trait]
impl Kaspawalletd for Service {
    async fn get_balance(&self, _request: Request<GetBalanceRequest>) -> Result<Response<GetBalanceResponse>, Status> {
        let balances = self.descriptor().balance.unwrap();
        let response = GetBalanceResponse { available: balances.mature, pending: balances.pending, address_balances: vec![] };
        Ok(Response::new(response))
    }

    async fn get_external_spendable_utx_os(
        &self,
        _request: Request<GetExternalSpendableUtxOsRequest>,
    ) -> Result<Response<GetExternalSpendableUtxOsResponse>, Status> {
        let address = Address::try_from(_request.get_ref().address.clone())
            .map_err(|_| Status::new(tonic::Code::InvalidArgument, "Invalid address provided"))?;
        let request = AccountsGetUtxosRequest {
            account_id: self.descriptor().account_id,
            addresses: Some(vec![address]),
            min_amount_sompi: None,
        };
        let utxos = self.wallet().accounts_get_utxos(request).await.unwrap().utxos;
        let response = GetExternalSpendableUtxOsResponse { entries: utxos.into_iter().map(Into::into).collect() };
        Ok(Response::new(response))
    }

    async fn create_unsigned_transactions(
        &self,
        _request: Request<CreateUnsignedTransactionsRequest>,
    ) -> Result<Response<CreateUnsignedTransactionsResponse>, Status> {
        todo!();
    }

    async fn show_addresses(&self, _request: Request<ShowAddressesRequest>) -> Result<Response<ShowAddressesResponse>, Status> {
        let addresses = self.receive_addresses().iter().map(|addr| addr.to_string()).collect::<Vec<String>>();
        let response = ShowAddressesResponse { address: addresses };
        Ok(Response::new(response))
    }

    async fn new_address(&self, _request: Request<NewAddressRequest>) -> Result<Response<NewAddressResponse>, Status> {
        let address =
            self.wallet().accounts_create_new_address(self.descriptor().account_id, NewAddressKind::Receive).await.unwrap().address;
        let response = NewAddressResponse { address: address.to_string() };
        Ok(Response::new(response))
    }

    async fn shutdown(&self, _request: Request<ShutdownRequest>) -> Result<Response<ShutdownResponse>, Status> {
        self.initiate_shutdown();
        Ok(Response::new(ShutdownResponse {}))
    }

    async fn broadcast(&self, request: Request<BroadcastRequest>) -> Result<Response<BroadcastResponse>, Status> {
        let request = request.into_inner();
        let txs: Vec<Result<RpcTransaction, Status>> = request.transactions.into_iter().map(|tx| -> Result<_, Status> {
            if request.is_domain {
                let tx = TransactionMessage::decode(tx.as_slice()).map_err(|err| Status::invalid_argument(err.to_string()))?;
                let tx = RpcTransaction::try_from(tx)?;
                Ok(tx)
            } else {
                todo!()
            }
        }).collect();
        let mut tx_ids: Vec<String> = Vec::with_capacity(txs.len());
        for (i, tx) in txs.iter().enumerate() {
            match tx {
                Ok(rpc) => {
                    let tx_id = self.wallet().rpc_api().submit_transaction(rpc.clone(), false).await;
                    match tx_id {
                        Ok(tx_id) => {
                            tx_ids[i] = tx_id.to_string();
                        },
                        Err(_) => {
                            todo!()
                        }
                    }
                }
                Err(_) => {
                    todo!()
                }
            }
        };
        // let a = self.wallet().rpc_api().submit_transaction();
        Ok(Response::new(BroadcastResponse { tx_ids }))
    }

    async fn broadcast_replacement(&self, _request: Request<BroadcastRequest>) -> Result<Response<BroadcastResponse>, Status> {
        todo!();
    }

    async fn send(&self, _request: Request<SendRequest>) -> Result<Response<SendResponse>, Status> {
        let data = _request.get_ref();
        let fee_rate_estimate = self.wallet().fee_rate_estimate().await.unwrap();
        let fee_rate = data.fee_policy.and_then(|policy| match policy.fee_policy.unwrap() {
            FeePolicy::MaxFeeRate(rate) => Some(fee_rate_estimate.normal.feerate.min(rate)),
            FeePolicy::ExactFeeRate(rate) => Some(rate),
            _ => None, // TODO: we dont support maximum_amount policy so think if we should supply default fee_rate_estimate or just 1 on this case...
        });
        let request = AccountsSendRequest {
            account_id: self.descriptor().account_id,
            wallet_secret: data.password.clone().into(),
            payment_secret: None,
            destination: PaymentDestination::PaymentOutputs(PaymentOutputs::from((
                Address::try_from(data.to_address.clone()).unwrap(),
                data.amount,
            ))),
            fee_rate,
            priority_fee_sompi: Fees::SenderPays(0),
            payload: None,
        };
        let result = self
            .wallet()
            .accounts_send(request)
            .await
            .map_err(|err| Status::new(tonic::Code::Internal, format!("Generator: {}", err)))?;
        let final_transaction = result.final_transaction_id.unwrap().to_string();
        let response = SendResponse { tx_ids: vec![final_transaction], signed_transactions: vec![] };
        Ok(Response::new(response))
    }

    async fn sign(&self, _request: Request<SignRequest>) -> Result<Response<SignResponse>, Status> {
        todo!();
    }

    async fn get_version(&self, _request: Request<GetVersionRequest>) -> Result<Response<GetVersionResponse>, Status> {
        let response = GetVersionResponse { version: env!("CARGO_PKG_VERSION").to_string() };
        Ok(Response::new(response))
    }

    async fn bump_fee(&self, _request: Request<BumpFeeRequest>) -> Result<Response<BumpFeeResponse>, Status> {
        todo!();
    }
}
