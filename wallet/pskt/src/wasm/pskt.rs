use crate::pskt::{SignInputOk, Signature, PSKT as Native};
use crate::role::*;
use kaspa_consensus_core::hashing::sighash::{calc_schnorr_signature_hash, SigHashReusedValues};
use kaspa_consensus_core::tx::{PopulatedTransaction, TransactionId};
use kaspa_txscript::get_sig_op_count;
use kaspa_txscript::opcodes::codes::OpData65;
use kaspa_txscript::script_builder::ScriptBuilder;
use wasm_bindgen::prelude::*;
// use js_sys::Object;
use crate::pskt::Inner;
use core::result::Result as CoreResult;
use kaspa_consensus_client::{Transaction, TransactionInput, TransactionInputT, TransactionOutput, TransactionOutputT};
use kaspa_wallet_keys::privatekey::PrivateKey;
use serde::{Deserialize, Serialize};
use std::iter;
use std::sync::MutexGuard;
use std::sync::{Arc, Mutex};
use workflow_wasm::{
    convert::{Cast, CastFromJs, TryCastFromJs},
    // extensions::object::*,
    // error::Error as CastError,
};

use super::error::*;
use super::result::*;

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "state", content = "payload")]
pub enum State {
    NoOp(Option<Inner>),
    Creator(Native<Creator>),
    Constructor(Native<Constructor>),
    Updater(Native<Updater>),
    Signer(Native<Signer>),
    Combiner(Native<Combiner>),
    Finalizer(Native<Finalizer>),
    Extractor(Native<Extractor>),
}

impl AsRef<State> for State {
    fn as_ref(&self) -> &State {
        self
    }
}

impl State {
    // this is not a Display trait intentionally
    pub fn display(&self) -> &'static str {
        match self {
            State::NoOp(_) => "Init",
            State::Creator(_) => "Creator",
            State::Constructor(_) => "Constructor",
            State::Updater(_) => "Updater",
            State::Signer(_) => "Signer",
            State::Combiner(_) => "Combiner",
            State::Finalizer(_) => "Finalizer",
            State::Extractor(_) => "Extractor",
        }
    }
}

impl From<State> for PSKT {
    fn from(state: State) -> Self {
        PSKT { state: Arc::new(Mutex::new(Some(state))) }
    }
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(typescript_type = "PSKT | Transaction")]
    pub type PayloadT;
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Payload {
    data: String,
}

impl<T> TryFrom<Payload> for Native<T> {
    type Error = Error;

    fn try_from(value: Payload) -> Result<Self> {
        let Payload { data } = value;
        if data.starts_with("PSKT") {
            unimplemented!("PSKT binary serialization")
        } else {
            Ok(serde_json::from_str(&data).map_err(|err| format!("Invalid JSON: {err}"))?)
        }
    }
}

#[wasm_bindgen(inspectable)]
#[derive(Clone, CastFromJs)]
pub struct PSKT {
    state: Arc<Mutex<Option<State>>>,
}

impl TryCastFromJs for PSKT {
    type Error = Error;
    fn try_cast_from<'a, R>(value: &'a R) -> std::result::Result<Cast<Self>, Self::Error>
    where
        R: AsRef<JsValue> + 'a,
    {
        Self::resolve(value, || {
            if let Some(data) = value.as_ref().as_string() {
                let pskt_inner: Inner = serde_json::from_str(&data).map_err(|_| Error::InvalidPayload)?;
                Ok(PSKT::from(State::NoOp(Some(pskt_inner))))
            } else if let Ok(transaction) = Transaction::try_owned_from(value) {
                let pskt_inner: Inner = transaction.try_into()?;
                Ok(PSKT::from(State::NoOp(Some(pskt_inner))))
            } else {
                Err(Error::InvalidPayload)
            }
        })
    }
}

#[wasm_bindgen]
impl PSKT {
    #[wasm_bindgen(constructor)]
    pub fn new(payload: Option<PayloadT>) -> Result<PSKT> {
        match payload {
            Some(payload) => {
                PSKT::try_owned_from(payload.unchecked_into::<JsValue>().as_ref()).map_err(|err| Error::Ctor(err.to_string()))
            }
            None => Ok(PSKT::from(State::NoOp(None))),
        }
    }

    #[wasm_bindgen(getter, js_name = "role")]
    pub fn role(&self) -> String {
        self.state().as_ref().unwrap().display().to_string()
    }

    #[wasm_bindgen(getter, js_name = "payload")]
    pub fn payload(&self) -> JsValue {
        // TODO: correctly typing
        let state = self.state();
        serde_wasm_bindgen::to_value(state.as_ref().unwrap()).unwrap()
    }

    /// Changes role to `CREATOR`. This initializes a PSKT in the Creator role,
    /// which is responsible for generating a new transaction without any signatures.
    #[wasm_bindgen(js_name = "toCreator")]
    pub fn creator(&self) -> Result<PSKT> {
        let state = match self.take() {
            State::NoOp(inner) => match inner {
                None => State::Creator(Native::default()),
                Some(_) => Err(Error::CreateNotAllowed)?,
            },
            state => Err(Error::state(state))?,
        };

        self.replace(state)
    }

    /// Changes role to `CONSTRUCTOR`. The constructor role is responsible for
    /// adding the necessary witness data, scripts, or other PSKT fields required
    /// to build the transaction. This role extends the creation phase, filling in
    /// additional transaction details.
    /// The constructor typically defines the transaction inputs and outputs.
    #[wasm_bindgen(js_name = "toConstructor")]
    pub fn constructor(&self) -> Result<PSKT> {
        let state = match self.take() {
            State::NoOp(inner) => State::Constructor(inner.ok_or(Error::NotInitialized)?.into()),
            State::Creator(pskt) => State::Constructor(pskt.constructor()),
            state => Err(Error::state(state))?,
        };

        self.replace(state)
    }

    #[wasm_bindgen(js_name = "addInput")]
    pub fn input(&self, input: &TransactionInputT) -> Result<PSKT> {
        let input = TransactionInput::try_owned_from(input)?;
        let state = match self.take() {
            State::Constructor(pskt) => State::Constructor(pskt.input(input.try_into()?)),
            state => Err(Error::state(state))?,
        };

        self.replace(state)
    }

    #[wasm_bindgen(js_name = "addOutput")]
    pub fn output(&self, output: &TransactionOutputT) -> Result<PSKT> {
        let output = TransactionOutput::try_owned_from(output)?;
        let state = match self.take() {
            State::Constructor(pskt) => State::Constructor(pskt.output(output.try_into()?)),
            state => Err(Error::state(state))?,
        };

        self.replace(state)
    }

    /// Changes role to `UPDATER`. The updater is responsible for filling in more
    /// specific information into the PSKT, such as completing any missing fields
    /// like sequence, and ensuring inputs are correctly referenced.
    #[wasm_bindgen(js_name = "toUpdater")]
    pub fn updater(&self) -> Result<PSKT> {
        let state = match self.take() {
            State::NoOp(inner) => State::Updater(inner.ok_or(Error::NotInitialized)?.into()),
            State::Constructor(constructor) => State::Updater(constructor.updater()),
            state => Err(Error::state(state))?,
        };

        self.replace(state)
    }

    /// The sequence number determines when the input can be spent.
    #[wasm_bindgen(js_name = "setSequence")]
    pub fn set_sequence(&self, n: u64, input_index: usize) -> Result<PSKT> {
        let state = match self.take() {
            State::Updater(pskt) => State::Updater(pskt.set_sequence(n, input_index)?),
            state => Err(Error::state(state))?,
        };

        self.replace(state)
    }

    /// Changes role to `SIGNER`. The signer is responsible for providing valid
    /// cryptographic signatures on the inputs of the PSKT. This role ensures that
    /// the transaction is authenticated and can later be combined with other
    /// signatures, if necessary.
    #[wasm_bindgen(js_name = "toSigner")]
    pub fn signer(&self) -> Result<PSKT> {
        let state = match self.take() {
            State::NoOp(inner) => State::Signer(inner.ok_or(Error::NotInitialized)?.into()),
            State::Constructor(pskt) => State::Signer(pskt.signer()),
            State::Updater(pskt) => State::Signer(pskt.signer()),
            State::Combiner(pskt) => State::Signer(pskt.signer()),
            state => Err(Error::state(state))?,
        };

        self.replace(state)
    }

    /// Calculates the current transaction id,
    /// can only be executed by signers.
    #[wasm_bindgen(js_name = "calculateId")]
    pub fn calculate_id(&self) -> Result<TransactionId> {
        let state = self.state.lock().unwrap();
        match state.as_ref().unwrap() {
            State::Signer(pskt) => Ok(pskt.calculate_id()),
            state => Err(Error::state(state))?,
        }
    }

    /// Signs all inputs and adds it to partial signature cache
    /// to be used in future, can only be executed by signers.
    #[wasm_bindgen(js_name = "sign")]
    pub fn sign(&self, private_key: &PrivateKey) -> Result<PSKT> {
        let state = match self.take() {
            State::Signer(pskt) => {
                let keypair = secp256k1::Keypair::from_seckey_slice(secp256k1::SECP256K1, &private_key.secret_bytes()).unwrap();
                let mut reused_values = SigHashReusedValues::new();

                let signed_pskt = pskt
                    .pass_signature_sync(|tx, sighash| -> CoreResult<Vec<SignInputOk>, String> {
                        tx.tx
                            .inputs
                            .iter()
                            .enumerate()
                            .map(|(idx, _input)| {
                                let hash = calc_schnorr_signature_hash(&tx.as_verifiable(), idx, sighash[idx], &mut reused_values);
                                let msg = secp256k1::Message::from_digest_slice(hash.as_bytes().as_slice()).unwrap();
                                Ok(SignInputOk {
                                    signature: Signature::Schnorr(keypair.sign_schnorr(msg)),
                                    pub_key: keypair.public_key(),
                                    key_source: None,
                                })
                            })
                            .collect()
                    })
                    .unwrap();
                State::Signer(signed_pskt)
            }
            state => Err(Error::state(state))?,
        };

        self.replace(state)
    }

    /// Changes role to `COMBINER`. The combiner merges multiple PSKTs from various
    /// signers into a single, cohesive PSKT. This role is responsible for ensuring
    /// that all necessary signatures are included and the transaction is ready for
    /// finalization.
    #[wasm_bindgen(js_name = "toCombiner")]
    pub fn combiner(&self) -> Result<PSKT> {
        let state = match self.take() {
            State::NoOp(inner) => State::Combiner(inner.ok_or(Error::NotInitialized)?.into()),
            State::Constructor(pskt) => State::Combiner(pskt.combiner()),
            State::Updater(pskt) => State::Combiner(pskt.combiner()),
            State::Signer(pskt) => State::Combiner(pskt.combiner()),
            state => Err(Error::state(state))?,
        };

        self.replace(state)
    }

    /// Combine an external signer PSKT into current PSKT by combiner role.
    /// The state of both PSKTs will reset.
    #[wasm_bindgen(js_name = "combine")]
    pub fn combine(&self, target_pskt: PSKT) -> Result<PSKT> {
        let state = match self.take() {
            State::Combiner(pskt) => match target_pskt.take() {
                State::Signer(other_pskt) => State::Combiner((pskt + other_pskt).unwrap()),
                state => Err(Error::state(state))?,
            },
            state => Err(Error::state(state))?,
        };

        self.replace(state)
    }

    /// Changes role to `FINALIZER`. The finalizer role is responsible for taking
    /// the fully signed PSKT and ensuring that the transaction is complete and
    /// ready to be submitted to the network.
    #[wasm_bindgen(js_name = "toFinalizer")]
    pub fn finalizer(&self) -> Result<PSKT> {
        let state = match self.take() {
            State::NoOp(inner) => State::Finalizer(inner.ok_or(Error::NotInitialized)?.into()), // TODO: possibly allow signer to also be a finalizer -- skipping combiner
            State::Combiner(pskt) => State::Finalizer(pskt.finalizer()),
            state => Err(Error::state(state))?,
        };

        self.replace(state)
    }

    /// Finalizes the partial signatures cache and populates the inputs with the 
    /// appropriate unlock scripts, ensuring that only the required number of 
    /// signatures (based on the sigOpCount) are included.
    #[wasm_bindgen(js_name = "finalize")]
    pub fn finalize(&self) -> Result<PSKT> {
        let state = match self.take() {
            State::Finalizer(pskt) => {
                let finalized_pskt = pskt
                    .finalize_sync(|inner: &Inner| -> CoreResult<Vec<Vec<u8>>, String> {
                        Ok(inner
                            .inputs
                            .iter()
                            .map(|input| -> Vec<u8> {
                                let required_sig_count = get_sig_op_count::<PopulatedTransaction>(
                                    &input.redeem_script.as_ref().unwrap(), // TODO: a question for aspect -- abt how to properly handle here
                                    &input.utxo_entry.as_ref().unwrap().script_public_key,
                                );

                                let signatures: Vec<_> = input
                                    .partial_sigs
                                    .iter()
                                    .take(required_sig_count as usize)
                                    .flat_map(|(_, signature)| {
                                        let sig_bytes = signature.into_bytes();
                                        iter::once(OpData65).chain(sig_bytes).chain([input.sighash_type.to_u8()])
                                    })
                                    .collect();

                                signatures
                                    .into_iter()
                                    .chain(
                                        ScriptBuilder::new()
                                            .add_data(input.redeem_script.as_ref().unwrap().as_slice())
                                            .unwrap()
                                            .drain()
                                            .iter()
                                            .cloned(),
                                    )
                                    .collect()
                            })
                            .collect())
                    })
                    .unwrap();

                State::Finalizer(finalized_pskt)
            }
            state => Err(Error::state(state))?,
        };

        self.replace(state)
    }

    /// Changes role to `EXTRACTOR`. The extractor is responsible for taking the
    /// final transaction from the PSKT and usually submitting it to network.
    #[wasm_bindgen(js_name = "toExtractor")]
    pub fn extractor(&self) -> Result<PSKT> {
        let state = match self.take() {
            State::NoOp(inner) => State::Extractor(inner.ok_or(Error::NotInitialized)?.into()),
            State::Finalizer(pskt) => State::Extractor(pskt.extractor()?),
            state => Err(Error::state(state))?,
        };

        self.replace(state)
    }

    /// If the transaction is finalized, this function extracts it.
    /// The state will reset after the transaction is extracted.
    #[wasm_bindgen(js_name = "extractTransaction")]
    pub fn extract_tx(&self) -> Result<Transaction> {
        match self.take() {
            State::Extractor(pskt) => Ok(pskt.extract_tx().unwrap()(0).0.into()),
            state => Err(Error::state(state))?,
        }
    }

    /// This allows specifying a lock time that will be used if no other lock time requirement  
    /// is set in the final transactions inputs.
    #[wasm_bindgen(js_name = "setFallbackLockTime")]
    pub fn fallback_lock_time(&self, lock_time: u64) -> Result<PSKT> {
        let state = match self.take() {
            State::Creator(pskt) => State::Creator(pskt.fallback_lock_time(lock_time)),
            state => Err(Error::state(state))?,
        };

        self.replace(state)
    }

    /// Marks the inputs as modifiable.
    #[wasm_bindgen(js_name = "makeInputsModifiable")]
    pub fn inputs_modifiable(&self) -> Result<PSKT> {
        let state = match self.take() {
            State::Creator(pskt) => State::Creator(pskt.inputs_modifiable()),
            state => Err(Error::state(state))?,
        };

        self.replace(state)
    }

    /// Marks the outputs as modifiable.
    #[wasm_bindgen(js_name = "makeOutputsModifiable")]
    pub fn outputs_modifiable(&self) -> Result<PSKT> {
        let state = match self.take() {
            State::Creator(pskt) => State::Creator(pskt.outputs_modifiable()),
            state => Err(Error::state(state))?,
        };

        self.replace(state)
    }

    /// Marks the inputs as finalized, preventing any additional inputs from being added.
    #[wasm_bindgen(js_name = "noMoreInputs")]
    pub fn no_more_inputs(&self) -> Result<PSKT> {
        let state = match self.take() {
            State::Constructor(pskt) => State::Constructor(pskt.no_more_inputs()),
            state => Err(Error::state(state))?,
        };

        self.replace(state)
    }

    /// Marks the outputs as finalized, preventing any additional outputs from being added.
    #[wasm_bindgen(js_name = "noMoreOutputs")]
    pub fn no_more_outputs(&self) -> Result<PSKT> {
        let state = match self.take() {
            State::Constructor(pskt) => State::Constructor(pskt.no_more_outputs()),
            state => Err(Error::state(state))?,
        };

        self.replace(state)
    }

    fn state(&self) -> MutexGuard<Option<State>> {
        self.state.lock().unwrap()
    }

    fn take(&self) -> State {
        self.state.lock().unwrap().take().unwrap()
    }

    fn replace(&self, state: State) -> Result<PSKT> {
        self.state.lock().unwrap().replace(state);
        Ok(self.clone())
    }
}
