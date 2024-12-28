//!
//! Tools for interfacing wallet accounts with PSKBs.
//! (Partial Signed Kaspa Transaction Bundles).
//!

pub use crate::error::Error;
use crate::imports::*;
use crate::tx::PaymentOutput;
use crate::tx::PaymentOutputs;
use futures::stream;
use kaspa_bip32::{DerivationPath, KeyFingerprint, PrivateKey};
use kaspa_consensus_client::UtxoEntry as ClientUTXO;
use kaspa_consensus_core::hashing::sighash::{calc_schnorr_signature_hash, SigHashReusedValuesUnsync};
use kaspa_consensus_core::tx::VerifiableTransaction;
use kaspa_consensus_core::tx::{TransactionInput, UtxoEntry};
use kaspa_txscript::extract_script_pub_key_address;
use kaspa_txscript::opcodes::codes::OpData65;
use kaspa_txscript::script_builder::ScriptBuilder;
use kaspa_wallet_pskt::bundle::script_sig_to_address;
use kaspa_wallet_pskt::prelude::unlock_utxo_as_batch_transaction_pskb;

use kaspa_wallet_core::tx::{Generator, GeneratorSettings, PaymentDestination, PendingTransaction};
pub use kaspa_wallet_pskt::bundle::Bundle;
use kaspa_wallet_pskt::prelude::lock_script_sig_templating_bytes;
use kaspa_wallet_pskt::prelude::KeySource;
use kaspa_wallet_pskt::prelude::{Finalizer, Inner, SignInputOk, Signature, Signer};
pub use kaspa_wallet_pskt::pskt::{Creator, PSKT};
use secp256k1::schnorr;
use secp256k1::{Message, PublicKey};
use std::iter;

struct PSKBSignerInner {
    keydata: PrvKeyData,
    account: Arc<dyn Account>,
    payment_secret: Option<Secret>,
    keys: Mutex<AHashMap<Address, [u8; 32]>>,
}

pub struct PSKBSigner {
    inner: Arc<PSKBSignerInner>,
}

impl PSKBSigner {
    pub fn new(account: Arc<dyn Account>, keydata: PrvKeyData, payment_secret: Option<Secret>) -> Self {
        Self { inner: Arc::new(PSKBSignerInner { keydata, account, payment_secret, keys: Mutex::new(AHashMap::new()) }) }
    }

    pub fn ingest(&self, addresses: &[Address]) -> Result<()> {
        let mut keys = self.inner.keys.lock()?;

        // Skip addresses that are already present in the key map.
        let addresses = addresses.iter().filter(|a| !keys.contains_key(a)).collect::<Vec<_>>();
        if !addresses.is_empty() {
            let account = self.inner.account.clone().as_derivation_capable().expect("expecting derivation capable account");
            let (receive, change) = account.derivation().addresses_indexes(&addresses)?;
            let private_keys = account.create_private_keys(&self.inner.keydata, &self.inner.payment_secret, &receive, &change)?;
            for (address, private_key) in private_keys {
                keys.insert(address.clone(), private_key.to_bytes());
            }
        }
        Ok(())
    }

    fn public_key(&self, for_address: &Address) -> Result<PublicKey> {
        let keys = self.inner.keys.lock()?;
        match keys.get(for_address) {
            Some(private_key) => {
                let kp = secp256k1::Keypair::from_seckey_slice(secp256k1::SECP256K1, private_key)?;
                Ok(kp.public_key())
            }
            None => Err(Error::from("PSKBSigner address coverage error")),
        }
    }

    fn sign_schnorr(&self, for_address: &Address, message: Message) -> Result<schnorr::Signature> {
        let keys = self.inner.keys.lock()?;
        match keys.get(for_address) {
            Some(private_key) => {
                let schnorr_key = secp256k1::Keypair::from_seckey_slice(secp256k1::SECP256K1, private_key)?;
                Ok(schnorr_key.sign_schnorr(message))
            }
            None => Err(Error::from("PSKBSigner address coverage error")),
        }
    }
}

pub struct PSKTGenerator {
    generator: Generator,
    signer: Arc<PSKBSigner>,
    prefix: Prefix,
}

impl PSKTGenerator {
    pub fn new(generator: Generator, signer: Arc<PSKBSigner>, prefix: Prefix) -> Self {
        Self { generator, signer, prefix }
    }

    pub fn stream(&self) -> impl Stream<Item = Result<PSKT<Signer>, Error>> {
        PSKTStream::new(self.generator.clone(), self.signer.clone(), self.prefix)
    }
}

struct PSKTStream {
    generator_stream: Pin<Box<dyn Stream<Item = Result<PendingTransaction, Error>> + Send>>,
    signer: Arc<PSKBSigner>,
    prefix: Prefix,
}

impl PSKTStream {
    fn new(generator: Generator, signer: Arc<PSKBSigner>, prefix: Prefix) -> Self {
        let generator_stream = generator.stream().map_err(Error::from);
        Self { generator_stream: Box::pin(generator_stream), signer, prefix }
    }
}

impl Stream for PSKTStream {
    type Item = Result<PSKT<Signer>, Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.as_ref();

        let _prefix = this.prefix;
        let _signer = this.signer.clone();

        match self.get_mut().generator_stream.as_mut().poll_next(cx) {
            Poll::Ready(Some(Ok(pending_tx))) => {
                let pskt = convert_pending_tx_to_pskt(pending_tx);
                Poll::Ready(Some(pskt))
            }
            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(e))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

fn convert_pending_tx_to_pskt(pending_tx: PendingTransaction) -> Result<PSKT<Signer>, Error> {
    let signable_tx = pending_tx.signable_transaction();
    let verifiable_tx = signable_tx.as_verifiable();
    let populated_inputs: Vec<(&TransactionInput, &UtxoEntry)> = verifiable_tx.populated_inputs().collect();
    let pskt_inner = Inner::try_from((pending_tx.transaction(), populated_inputs.to_owned()))?;
    Ok(PSKT::<Signer>::from(pskt_inner))
}

pub async fn bundle_from_pskt_generator(generator: PSKTGenerator) -> Result<Bundle, Error> {
    let mut bundle: Bundle = Bundle::new();
    let mut stream = generator.stream();

    while let Some(pskt_result) = stream.next().await {
        match pskt_result {
            Ok(pskt) => bundle.add_pskt(pskt),
            Err(e) => return Err(e),
        }
    }

    Ok(bundle)
}

pub async fn pskb_signer_for_address(
    bundle: &Bundle,
    signer: Arc<PSKBSigner>,
    network_id: NetworkId,
    sign_for_address: Option<&Address>,
    derivation_path: DerivationPath,
    key_fingerprint: KeyFingerprint,
) -> Result<Bundle, Error> {
    let mut signed_bundle = Bundle::new();
    let reused_values = SigHashReusedValuesUnsync::new();

    // If set, sign-for address is used for signing.
    // Else, all addresses from inputs are.
    let addresses: Vec<Address> = match sign_for_address {
        Some(signer) => vec![signer.clone()],
        None => bundle
            .iter()
            .flat_map(|inner| {
                inner.inputs
                    .iter()
                    .filter_map(|input| input.utxo_entry.as_ref()) // Filter out None and get a reference to UtxoEntry if it exists
                    .filter_map(|utxo_entry| {
                        extract_script_pub_key_address(&utxo_entry.script_public_key.clone(), network_id.into()).ok()
                    })
                    .collect::<Vec<Address>>()
            })
            .collect(),
    };

    // Prepare the signer.
    signer.ingest(addresses.as_ref())?;

    for pskt_inner in bundle.iter().cloned() {
        let pskt: PSKT<Signer> = PSKT::from(pskt_inner);

        let sign = |signer_pskt: PSKT<Signer>| {
            signer_pskt
                .pass_signature_sync(|tx, sighash| -> Result<Vec<SignInputOk>, String> {
                    tx.tx
                        .inputs
                        .iter()
                        .enumerate()
                        .map(|(idx, _input)| {
                            let hash = calc_schnorr_signature_hash(&tx.as_verifiable(), idx, sighash[idx], &reused_values);
                            let msg = secp256k1::Message::from_digest_slice(hash.as_bytes().as_slice()).unwrap();

                            // When address represents a locked UTXO, no private key is available.
                            // Instead, use the account receive address' private key.
                            let address: &Address = match sign_for_address {
                                Some(address) => address,
                                None => addresses.get(idx).expect("Input indexed address"),
                            };

                            let public_key = signer.public_key(address).expect("Public key for input indexed address");

                            Ok(SignInputOk {
                                signature: Signature::Schnorr(signer.sign_schnorr(address, msg).unwrap()),
                                pub_key: public_key,
                                key_source: Some(KeySource { key_fingerprint, derivation_path: derivation_path.clone() }),
                            })
                        })
                        .collect()
                })
                .unwrap()
        };
        signed_bundle.add_pskt(sign(pskt.clone()));
    }
    Ok(signed_bundle)
}

pub fn finalize_pskt_one_or_more_sig_and_redeem_script(pskt: PSKT<Finalizer>) -> Result<PSKT<Finalizer>, Error> {
    let result = pskt.finalize_sync(|inner: &Inner| -> Result<Vec<Vec<u8>>, String> {
        Ok(inner
            .inputs
            .iter()
            .map(|input| -> Vec<u8> {
                let signatures: Vec<_> = input
                    .partial_sigs
                    .clone()
                    .into_iter()
                    .flat_map(|(_, signature)| iter::once(OpData65).chain(signature.into_bytes()).chain([input.sighash_type.to_u8()]))
                    .collect();

                signatures
                    .into_iter()
                    .chain(
                        input
                            .redeem_script
                            .as_ref()
                            .map(|redeem_script| ScriptBuilder::new().add_data(redeem_script.as_slice()).unwrap().drain().to_vec())
                            .unwrap_or_default(),
                    )
                    .collect()
            })
            .collect())
    });

    match result {
        Ok(finalized_pskt) => Ok(finalized_pskt),
        Err(e) => Err(Error::from(e.to_string())),
    }
}

pub fn finalize_pskt_no_sig_and_redeem_script(pskt: PSKT<Finalizer>) -> Result<PSKT<Finalizer>, Error> {
    let result = pskt.finalize_sync(|inner: &Inner| -> Result<Vec<Vec<u8>>, String> {
        Ok(inner
            .inputs
            .iter()
            .map(|input| -> Vec<u8> {
                input
                    .redeem_script
                    .as_ref()
                    .map(|redeem_script| ScriptBuilder::new().add_data(redeem_script.as_slice()).unwrap().drain().to_vec())
                    .unwrap_or_default()
            })
            .collect())
    });

    match result {
        Ok(finalized_pskt) => Ok(finalized_pskt),
        Err(e) => Err(Error::from(e.to_string())),
    }
}

pub fn bundle_to_finalizer_stream(bundle: &Bundle) -> impl Stream<Item = Result<PSKT<Finalizer>, Error>> + Send {
    stream::iter(bundle.iter().cloned().collect::<Vec<_>>()).map(move |pskt_inner| {
        let pskt: PSKT<Creator> = PSKT::from(pskt_inner);
        let pskt_finalizer = pskt.constructor().updater().signer().finalizer();
        finalize_pskt_one_or_more_sig_and_redeem_script(pskt_finalizer)
    })
}

pub fn pskt_to_pending_transaction(
    finalized_pskt: PSKT<Finalizer>,
    network_id: NetworkId,
    change_address: Address,
) -> Result<PendingTransaction, Error> {
    let mass = 10;
    let (signed_tx, _) = match finalized_pskt.clone().extractor() {
        Ok(extractor) => match extractor.extract_tx() {
            Ok(once_mass) => once_mass(mass),
            Err(e) => return Err(Error::PendingTransactionFromPSKTError(e.to_string())),
        },
        Err(e) => return Err(Error::PendingTransactionFromPSKTError(e.to_string())),
    };

    let inner_pskt = finalized_pskt.deref().clone();

    let utxo_entries_ref: Vec<UtxoEntryReference> = inner_pskt
        .inputs
        .iter()
        .filter_map(|input| {
            if let Some(ue) = input.clone().utxo_entry {
                return Some(UtxoEntryReference {
                    utxo: Arc::new(ClientUTXO {
                        address: Some(extract_script_pub_key_address(&ue.script_public_key, network_id.into()).unwrap()),
                        amount: ue.amount,
                        outpoint: input.previous_outpoint.into(),
                        script_public_key: ue.script_public_key,
                        block_daa_score: ue.block_daa_score,
                        is_coinbase: ue.is_coinbase,
                    }),
                });
            }
            None
        })
        .collect();

    let output: Vec<kaspa_consensus_core::tx::TransactionOutput> = signed_tx.outputs.clone();
    let recipient = extract_script_pub_key_address(&output[0].script_public_key, network_id.into())?;
    let fee_u: u64 = 0;

    let utxo_iterator: Box<dyn Iterator<Item = UtxoEntryReference> + Send + Sync + 'static> =
        Box::new(utxo_entries_ref.clone().into_iter());

    let final_transaction_destination = PaymentDestination::PaymentOutputs(PaymentOutputs::from((recipient.clone(), output[0].value)));

    let settings = GeneratorSettings {
        network_id,
        multiplexer: None,
        sig_op_count: 1,
        minimum_signatures: 1,
        change_address,
        utxo_iterator,
        priority_utxo_entries: None,
        source_utxo_context: None,
        destination_utxo_context: None,
        fee_rate: None,
        final_transaction_priority_fee: fee_u.into(),
        final_transaction_destination,
        final_transaction_payload: None,
    };

    // Create the Generator
    let generator = Generator::try_new(settings, None, None)?;

    // Create PendingTransaction (WIP)
    let pending_tx = PendingTransaction::try_new(
        &generator,
        signed_tx.clone(),
        utxo_entries_ref.clone(),
        vec![],
        None,
        None,
        0,
        0,
        0,
        1,
        0,
        0,
        kaspa_wallet_core::tx::DataKind::Final,
    )?;

    Ok(pending_tx)
}

// Allow creation of atomic commit reveal operation with two
// different parameters sets.
pub enum CommitRevealBatchKind {
    Manual { hop_payment: PaymentDestination, destination_payment: PaymentDestination },
    Parameterized { address: Address, commit_amount_sompi: u64 },
}

struct BundleCommitRevealConfig {
    pub address_commit: Address,
    pub address_reveal: Address,
    pub first_output: PaymentDestination,
    pub commit_fee: Option<u64>,
    pub reveal_fee: u64,
    pub redeem_script: Vec<u8>,
}

// Create signed atomic commit reveal PSKB.
// Default reveal fee of 100_000 sompi if priority_fee_sompi is not provided.
pub async fn commit_reveal_batch_bundle(
    batch_config: CommitRevealBatchKind,
    reveal_fee_sompi: Option<u64>,
    script_sig: Vec<u8>,
    payload: Option<Vec<u8>>,
    fee_rate: Option<f64>,
    account: Arc<dyn Account>,
    wallet_secret: Secret,
    payment_secret: Option<Secret>,
    abortable: &Abortable,
) -> Result<Bundle, Error> {
    let network_id = account.wallet().clone().network_id()?;

    // Configure atomic batch of commit reveal transactions relative to set of parameters.

    let mut conf: BundleCommitRevealConfig = match batch_config {
        CommitRevealBatchKind::Manual { hop_payment, destination_payment } => {
            let addr_commit = match hop_payment.clone() {
                PaymentDestination::Change => Err(()),
                PaymentDestination::PaymentOutputs(payment_outputs) => Ok(payment_outputs.outputs.first().unwrap().address.clone()),
            }
            .unwrap();

            let addr_reveal = match destination_payment.clone() {
                PaymentDestination::Change => Err(()),
                PaymentDestination::PaymentOutputs(payment_outputs) => Ok(payment_outputs.outputs.first().unwrap().address.clone()),
            }
            .unwrap();

            BundleCommitRevealConfig {
                address_commit: addr_commit,
                address_reveal: addr_reveal,
                first_output: hop_payment,
                commit_fee: None,
                reveal_fee: 100_000,
                redeem_script: script_sig,
            }
        }
        CommitRevealBatchKind::Parameterized { address, commit_amount_sompi } => {
            let redeem_script = lock_script_sig_templating_bytes(script_sig.to_vec(), Some(&address.payload))?;
            let lock_address = script_sig_to_address(&redeem_script, network_id.into())?;
            BundleCommitRevealConfig {
                address_commit: lock_address.clone(),
                address_reveal: address.clone(),
                first_output: PaymentDestination::from(PaymentOutput::new(lock_address, commit_amount_sompi)),
                commit_fee: None,
                reveal_fee: 100_000,
                redeem_script,
            }
        }
    };

    // Up to two optional priority fees can be set: if only the first one is set, it will
    // be applied to both transactions, whereas if both fees are set they will
    // respectively be applied to commit and reveal transaction.
    //
    // A default minimum reveal transaction fee is set to 1000_000.
    // conf.commit_fee = priority_fee_sompi.clone().and_then(|v| v.into_iter().next());
    // conf.reveal_fee = priority_fee_sompi.and_then(|v| v.into_iter().nth(1)).or(conf.commit_fee).or(Some(100_000));
    conf.reveal_fee = reveal_fee_sompi.unwrap_or(100_000);

    // Generate commit transaction.
    let settings = GeneratorSettings::try_new_with_account(
        account.clone().as_dyn_arc(),
        conf.first_output.clone(),
        fee_rate.or(Some(1.0)),
        conf.commit_fee.unwrap_or(0).into(),
        payload,
    )?;
    let signer = Arc::new(PSKBSigner::new(
        account.clone().as_dyn_arc(),
        account.prv_key_data(wallet_secret.clone()).await?,
        payment_secret.clone(),
    ));
    let generator = Generator::try_new(settings, None, Some(abortable))?;
    let pskt_generator = PSKTGenerator::new(generator, signer, account.wallet().address_prefix()?);

    let bundle_commit = bundle_from_pskt_generator(pskt_generator).await?;

    // Generate reveal transaction

    // todo: support priority fee.
    let bundle_unlock = unlock_utxo_as_batch_transaction_pskb(
        conf.first_output.amount().unwrap(),
        &conf.address_commit,
        &conf.address_reveal,
        &conf.redeem_script,
        Some(conf.reveal_fee),
    )?;

    let mut merge_bundle: Option<Bundle> = None;

    let commit_transaction_id =
        match account.clone().pskb_sign(&bundle_commit, wallet_secret.clone(), payment_secret.clone(), None).await {
            Ok(signed_pskb) => {
                merge_bundle = Some(Bundle::deserialize(&signed_pskb.serialize()?)?);

                let first_inner = signed_pskb.0.first().unwrap();
                let pskt: PSKT<Signer> = PSKT::<Signer>::from(first_inner.to_owned());
                let finalizer = pskt.finalizer();
                let pskt_finalizer = finalize_pskt_one_or_more_sig_and_redeem_script(finalizer)?;

                let commit_transaction_id = match pskt_to_pending_transaction(
                    pskt_finalizer.clone(),
                    network_id,
                    account.clone().as_derivation_capable()?.change_address()?,
                ) {
                    Ok(tx) => Ok(tx.id()),
                    Err(e) => Err(e),
                }?;
                Ok(commit_transaction_id)
            }
            Err(e) => Err(e),
        }?;

    // Set commit transaction ID in reveal batch transaction input.
    let reveal_inner = bundle_unlock.0.first().unwrap();
    let reveal_pskt: PSKT<Signer> = PSKT::<Signer>::from(reveal_inner.to_owned());

    let new_pskt = reveal_pskt.set_input_prev_transaction_id(commit_transaction_id);
    let unorphaned_bundle_unlock = Bundle::from(new_pskt);

    // Sign unlock transaction.
    if let Ok(signed_pskb) = account
        .clone()
        .pskb_sign(&unorphaned_bundle_unlock, wallet_secret.clone(), payment_secret.clone(), Some(&conf.address_reveal))
        .await
    {
        // Merge with commit transaction and return.
        if merge_bundle.is_some() {
            let mut bundle = merge_bundle.unwrap();
            bundle.merge(signed_pskb);
            return Ok(bundle);
        }
    }

    Err(Error::CommitRevealBatchGeneratorError)
}
