pub mod service;

use kaspa_consensus_core::tx::Transaction;
use kaspa_wallet_core::api::WalletApi;
use kaspa_wallet_core::{
    api::{AccountsGetUtxosRequest, NewAddressKind},
    prelude::Address,
};
use kaspa_wallet_grpc_core::convert::{deserialize_txs, extract_tx};
use kaspa_wallet_grpc_core::kaspawalletd::{
    kaspawalletd_server::Kaspawalletd, BroadcastRequest, BroadcastResponse, BumpFeeRequest, BumpFeeResponse,
    CreateUnsignedTransactionsRequest, CreateUnsignedTransactionsResponse, GetBalanceRequest, GetBalanceResponse,
    GetExternalSpendableUtxOsRequest, GetExternalSpendableUtxOsResponse, GetVersionRequest, GetVersionResponse, NewAddressRequest,
    NewAddressResponse, SendRequest, SendResponse, ShowAddressesRequest, ShowAddressesResponse, ShutdownRequest, ShutdownResponse,
    SignRequest, SignResponse,
};
use kaspa_wallet_grpc_core::protoserialization::PartiallySignedTransaction;
use service::Service;
use tonic::{Code, Request, Response, Status};

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
        request: Request<CreateUnsignedTransactionsRequest>,
    ) -> Result<Response<CreateUnsignedTransactionsResponse>, Status> {
        let CreateUnsignedTransactionsRequest { address, amount, from, use_existing_change_address, is_send_all, fee_policy } =
            request.into_inner();
        let unsigned_transactions =
            self.create_unsigned_transactions(address, amount, from, use_existing_change_address, is_send_all, fee_policy).await?;
        let unsigned_transactions =
            unsigned_transactions.into_iter().map(|tx| PartiallySignedTransaction::from_unsigned(tx).encode_to_vec()).collect();
        Ok(Response::new(CreateUnsignedTransactionsResponse { unsigned_transactions }))
    }

    async fn show_addresses(&self, _request: Request<ShowAddressesRequest>) -> Result<Response<ShowAddressesResponse>, Status> {
        let addresses = self.receive_addresses().iter().map(|addr| addr.to_string()).collect::<Vec<String>>();
        let response = ShowAddressesResponse { address: addresses };
        Ok(Response::new(response))
    }

    async fn new_address(&self, _request: Request<NewAddressRequest>) -> Result<Response<NewAddressResponse>, Status> {
        let address = self
            .wallet()
            .accounts_create_new_address(self.descriptor().account_id, NewAddressKind::Receive)
            .await
            .map_err(|err| Status::internal(err.to_string()))?
            .address;
        let response = NewAddressResponse { address: address.to_string() };
        Ok(Response::new(response))
    }

    async fn shutdown(&self, _request: Request<ShutdownRequest>) -> Result<Response<ShutdownResponse>, Status> {
        self.initiate_shutdown();
        Ok(Response::new(ShutdownResponse {}))
    }

    // TODO: Consider implementing parallel transaction processing in the future:
    // - Server-side configuration processes messages sequentially
    // - It might be possible to start processing a new message before writing the response to the socket
    // - New parameters like allow_parallel should be introduced
    // - Client behavior should be considered as they may expect sequential processing until the first error when sending batches
    async fn broadcast(&self, request: Request<BroadcastRequest>) -> Result<Response<BroadcastResponse>, Status> {
        let BroadcastRequest { transactions, is_domain } = request.into_inner();
        let tx_ids = self.broadcast(transactions, is_domain).await?;
        Ok(Response::new(BroadcastResponse { tx_ids }))
    }

    async fn broadcast_replacement(&self, request: Request<BroadcastRequest>) -> Result<Response<BroadcastResponse>, Status> {
        let request = request.into_inner();
        let txs = deserialize_txs(request.transactions, request.is_domain, self.use_ecdsa())?;
        let mut tx_ids: Vec<String> = Vec::with_capacity(txs.len());
        for (i, tx) in txs.into_iter().enumerate() {
            // Once the first transaction is added to the mempool, the transactions that depend
            // on the replaced transaction will be removed, so there's no need to submit them
            // as RBF transactions.
            let tx_id = if i == 0 {
                let submit_transaction_replacement_response = self
                    .wallet()
                    .rpc_api()
                    .submit_transaction_replacement(tx)
                    .await
                    .map_err(|e| Status::new(Code::Internal, e.to_string()))?;
                submit_transaction_replacement_response.transaction_id
            } else {
                self.wallet().rpc_api().submit_transaction(tx, false).await.map_err(|e| Status::new(Code::Internal, e.to_string()))?
            };
            tx_ids.push(tx_id.to_string());
        }
        Ok(Response::new(BroadcastResponse { tx_ids }))
    }

    async fn send(&self, request: Request<SendRequest>) -> Result<Response<SendResponse>, Status> {
        let SendRequest { to_address, amount, password, from, use_existing_change_address, is_send_all, fee_policy } =
            request.into_inner();
        let (signed_transactions, tx_ids) =
            self.send(to_address, amount, password, from, use_existing_change_address, is_send_all, fee_policy).await?;
        Ok(Response::new(SendResponse { tx_ids, signed_transactions }))
    }

    async fn sign(&self, request: Request<SignRequest>) -> Result<Response<SignResponse>, Status> {
        let SignRequest { unsigned_transactions, password } = request.into_inner();
        let deserialized = unsigned_transactions
            .iter()
            .map(|tx| extract_tx(tx.as_slice(), self.use_ecdsa()))
            // todo convert directly to consensus::transaction
            .map(|r| r
                .and_then(|rtx| Transaction::try_from(rtx)
                    .map_err(|err| Status::internal(err.to_string()))))
            .collect::<Result<Vec<_>, _>>()?;
        let signed_transactions = self.sign(deserialized, password).await?;
        Ok(Response::new(SignResponse { signed_transactions }))
    }

    async fn get_version(&self, _request: Request<GetVersionRequest>) -> Result<Response<GetVersionResponse>, Status> {
        let response = GetVersionResponse { version: env!("CARGO_PKG_VERSION").to_string() };
        Ok(Response::new(response))
    }

    async fn bump_fee(&self, _request: Request<BumpFeeRequest>) -> Result<Response<BumpFeeResponse>, Status> {
        // wallet api doesnt support RBF, requires manual implementation
        Err(Status::unimplemented("Bump fee is not implemented yet"))
    }
}
