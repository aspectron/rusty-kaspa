pub use crate::error::Error;
use crate::imports::*;
use crate::tx::PaymentOutputs;
use futures::stream;
use kaspa_bip32::PrivateKey;
use kaspa_consensus_client::UtxoEntry as ClientUTXO;
use kaspa_consensus_core::hashing::sighash::{calc_schnorr_signature_hash, SigHashReusedValues};
use kaspa_consensus_core::tx::VerifiableTransaction;
use kaspa_consensus_core::tx::{TransactionInput, UtxoEntry};
use kaspa_txscript::extract_script_pub_key_address;
use kaspa_txscript::opcodes::codes::OpData65;
use kaspa_txscript::script_builder::ScriptBuilder;
use kaspa_wallet_core::tx::{Generator, GeneratorSettings, PaymentDestination, PendingTransaction};
pub use kaspa_wallet_pskt::bundle::Bundle;
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
        let mut keys = self.inner.keys.lock().unwrap();
        // skip address that are already present in the key map
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
        let keys = self.inner.keys.lock().unwrap();
        match keys.get(for_address) {
            Some(private_key) => {
                let kp = secp256k1::Keypair::from_seckey_slice(secp256k1::SECP256K1, private_key).unwrap();
                Ok(kp.public_key())
            }
            None => Err(Error::from("PSKBSigner address coverage error")),
        }
    }

    fn sign_schnorr(&self, for_address: &Address, message: Message) -> Result<schnorr::Signature> {
        let keys = self.inner.keys.lock().unwrap();
        match keys.get(for_address) {
            Some(private_key) => {
                let schnorr_key = secp256k1::Keypair::from_seckey_slice(secp256k1::SECP256K1, private_key).unwrap();
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
        let this = self.as_ref(); // Access the mutable reference to self

        let _prefix = this.prefix;
        let _signer = this.signer.clone();

        match self.get_mut().generator_stream.as_mut().poll_next(cx) {
            Poll::Ready(Some(Ok(pending_tx))) => {
                let pskt = convert_pending_tx_to_pskt(pending_tx, false);
                Poll::Ready(Some(pskt))
            }
            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(e))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

fn convert_pending_tx_to_pskt(pending_tx: PendingTransaction, persist_signatures: bool) -> Result<PSKT<Signer>, Error> {
    let signable_tx = pending_tx.signable_transaction();

    let verifiable_tx = signable_tx.as_verifiable();

    let populated_inputs: Vec<(&TransactionInput, &UtxoEntry)> = verifiable_tx.populated_inputs().collect();

    let pskt_inner = Inner::from((pending_tx.transaction(), populated_inputs.to_owned()));

    let signer = PSKT::<Signer>::from(pskt_inner);

    // todo: detection and depending on PSKT type
    if persist_signatures {
        let signed_pskt = signer
            .pass_signature_sync(|tx, _| -> Result<Vec<SignInputOk>, String> {
                tx.tx
                    .inputs
                    .iter()
                    .enumerate()
                    .filter_map(|(idx, _input)| {
                        if let Some((input, _)) = signable_tx.as_verifiable().populated_inputs().nth(idx) {
                            if !input.signature_script.is_empty() {
                                let signature =
                                    secp256k1::schnorr::Signature::from_slice(&input.signature_script).expect("Schnorr signature");

                                // unsuported requirement: new public key as placeholder
                                // todo: optional or with more types
                                let secp = secp256k1::Secp256k1::new();
                                let keypair = secp256k1::Keypair::new(&secp, &mut rand::thread_rng());
                                let public_key = PublicKey::from_keypair(&keypair);

                                return Some(Ok(SignInputOk {
                                    signature: Signature::Schnorr(signature),
                                    pub_key: public_key,
                                    key_source: None,
                                }));
                            }
                        }
                        None
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .unwrap();
        return Ok(signed_pskt);
    }

    Ok(signer)
}

pub async fn bundle_from_pskt_generator(generator: PSKTGenerator) -> Result<Bundle, Error> {
    let mut bundle = Bundle::new();
    let mut stream = generator.stream();

    while let Some(pskt_result) = stream.next().await {
        match pskt_result {
            Ok(pskt) => bundle.add_pskt(pskt),
            Err(e) => return Err(e),
        }
    }

    Ok(bundle)
}

pub async fn pskb_signer(bundle: &Bundle, signer: Arc<PSKBSigner>, network_id: NetworkId) -> Result<Bundle, Error> {
    pskb_signer_for_address(bundle, signer, network_id, None).await
}

pub async fn pskb_signer_for_address(
    bundle: &Bundle,
    signer: Arc<PSKBSigner>,
    network_id: NetworkId,
    sign_for_address: Option<&Address>,
) -> Result<Bundle, Error> {
    let mut signed_bundle = Bundle::new();
    let mut reused_values = SigHashReusedValues::new();

    for pskt_inner in bundle.inner_list.clone() {
        let pskt: PSKT<Signer> = PSKT::from(pskt_inner);

        //
        let addresses: Vec<Address> = pskt.inputs.iter()
            .filter_map(|input| input.utxo_entry.as_ref()) // Filter out None and get a reference to UtxoEntry if it exists
            .map(|utxo_entry| extract_script_pub_key_address(&utxo_entry.script_public_key.clone(), network_id.into()).unwrap()) // Clone the ScriptPublicKey
            .collect();

        signer.ingest(addresses.as_ref())?;

        // Define the signing function
        let mut sign = |signer_pskt: PSKT<Signer>| {
            signer_pskt
                .pass_signature_sync(|tx, sighash| -> Result<Vec<SignInputOk>, String> {
                    tx.tx
                        .inputs
                        .iter()
                        .enumerate()
                        .map(|(idx, _input)| {
                            let hash = calc_schnorr_signature_hash(&tx.as_verifiable(), idx, sighash[idx], &mut reused_values);
                            let msg = secp256k1::Message::from_digest_slice(hash.as_bytes().as_slice()).unwrap();

                            let address: &Address = match sign_for_address {
                                Some(given_address) => given_address,
                                None => addresses.get(idx).expect("Input indexed address"),
                            };

                            // when address is a lock utxo, no private key will be available for that
                            // instead, use the account receive address private key

                            let public_key = signer.public_key(address).expect("Public key for input indexed address");

                            // sign for pubkeys the signer has the private key for

                            Ok(SignInputOk {
                                signature: Signature::Schnorr(signer.sign_schnorr(address, msg).unwrap()),
                                pub_key: public_key,
                                key_source: None,
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

pub fn bundle_to_finalizer_stream(bundle: &Bundle) -> Pin<Box<dyn Stream<Item = Result<PSKT<Finalizer>, Error>> + Send>> {
    let stream = stream::iter(bundle.inner_list.clone()).map(move |pskt_inner| {
        let pskt: PSKT<Creator> = PSKT::from(pskt_inner);
        let pskt_finalizer = pskt.constructor().updater().signer().finalizer();

        let result = pskt_finalizer.finalize_sync(|inner: &Inner| -> Result<Vec<Vec<u8>>, String> {
            Ok(inner
                .inputs
                .iter()
                .map(|input| -> Vec<u8> {
                    let signatures: Vec<_> = input
                        .partial_sigs
                        .clone()
                        .into_iter()
                        .flat_map(|(_, signature)| {
                            iter::once(OpData65).chain(signature.into_bytes()).chain([input.sighash_type.to_u8()])
                        })
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
    });

    Box::pin(stream)
}

// todo: discuss conversion to pending transaction
pub fn pskt_to_pending_transaction(
    finalized_pskt: PSKT<Finalizer>,
    network_id: NetworkId,
    change_address: Address,
) -> Result<PendingTransaction, Error> {
    let (signed_tx, _) = finalized_pskt.clone().extractor().unwrap().extract_tx().unwrap()(10);

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

    let utxo_iterator: Box<dyn Iterator<Item = UtxoEntryReference> + Send + Sync + 'static> =
        Box::new(utxo_entries_ref.clone().into_iter());

    let final_transaction_destination = PaymentDestination::PaymentOutputs(PaymentOutputs::from((recipient.clone(), output[0].value)));

    let fee_u: u64 = 0;

    let settings = GeneratorSettings {
        network_id,
        multiplexer: None,
        sig_op_count: 1,
        minimum_signatures: 1,
        change_address,
        utxo_iterator,
        source_utxo_context: None,
        destination_utxo_context: None,
        final_transaction_priority_fee: fee_u.into(),
        final_transaction_destination,
        final_transaction_payload: None,
    };

    // Create the Generator
    let generator = Generator::try_new(settings, None, None)?;

    // Create PendingTransaction
    let pending_tx = PendingTransaction::try_new(
        &generator,
        signed_tx.clone(),
        utxo_entries_ref.clone(),
        vec![],
        None,
        0,
        0,
        0,
        0,
        0,
        kaspa_wallet_core::tx::DataKind::Final,
    )?;

    Ok(pending_tx)
}
