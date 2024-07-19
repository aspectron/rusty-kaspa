use crate::imports::*;
use kaspa_consensus_core::tx::{TransactionOutpoint, UtxoEntry};
use kaspa_wallet_core::account::pskb::finalize_pskt_one_or_more_sig_and_redeem_script;
use kaspa_wallet_pskt::prelude::*;

#[derive(Default, Handler)]
#[help("Send a Kaspa transaction to a public address")]
pub struct Pskb;

impl Pskb {
    async fn main(self: Arc<Self>, ctx: &Arc<dyn Context>, mut argv: Vec<String>, _cmd: &str) -> Result<()> {
        let ctx = ctx.clone().downcast_arc::<KaspaCli>()?;

        let account = ctx.wallet().account()?;

        if argv.is_empty() {
            return self.display_help(ctx, argv).await;
        }

        let action = argv.remove(0);

        match action.as_str() {
            "create" => {
                if argv.len() < 2 || argv.len() > 3 {
                    return self.display_help(ctx, argv).await;
                } else {
                    let (wallet_secret, payment_secret) = ctx.ask_wallet_secret(None).await?;
                    let _ = ctx.notifier().show(Notification::Processing).await;

                    let address = Address::try_from(argv.first().unwrap().as_str())?;
                    let amount_sompi = try_parse_required_nonzero_kaspa_as_sompi_u64(argv.get(1))?;
                    let outputs = PaymentOutputs::from((address, amount_sompi));
                    let priority_fee_sompi = try_parse_optional_kaspa_as_sompi_i64(argv.get(2))?.unwrap_or(0);
                    let abortable = Abortable::default();

                    let signer = account
                        .pskb_from_send_generator(
                            outputs.into(),
                            priority_fee_sompi.into(),
                            None,
                            wallet_secret.clone(),
                            payment_secret.clone(),
                            &abortable,
                        )
                        .await?;

                    let pskb = signer.to_hex().unwrap();
                    tprintln!(ctx, "{pskb}");
                }
            }
            "script" => {
                if argv.len() < 2 || argv.len() > 4 {
                    return self.display_help(ctx, argv).await;
                }

                let subcommand = argv.remove(0);
                let payload = argv.remove(0);

                let receive_address = account.receive_address().unwrap(); // todo exception
                let (wallet_secret, payment_secret) = ctx.ask_wallet_secret(None).await?;
                let _ = ctx.notifier().show(Notification::Processing).await;

                let script_sig = match lock_script_sig(payload.clone(), Some(&receive_address.payload)) {
                    Ok(value) => value,
                    Err(e) => {
                        terrorln!(ctx, "{}", e.to_string());
                        return Err(e.into());
                    }
                };

                let script_p2sh = match script_addr(&script_sig, ctx.wallet().address_prefix()?) {
                    Ok(p2sh) => p2sh,
                    Err(e) => {
                        terrorln!(ctx, "Error generating script address: {}", e.to_string());
                        return Err(e.into());
                    }
                };

                match subcommand.as_str() {
                    "lock" => {
                        let amount_sompi = try_parse_required_nonzero_kaspa_as_sompi_u64(argv.first())?;
                        let outputs = PaymentOutputs::from((script_p2sh, amount_sompi));
                        let priority_fee_sompi = try_parse_optional_kaspa_as_sompi_i64(argv.get(1))?.unwrap_or(0);
                        let abortable = Abortable::default();

                        let signer = account
                            .pskb_from_send_generator(
                                outputs.into(),
                                priority_fee_sompi.into(),
                                None,
                                wallet_secret.clone(),
                                payment_secret.clone(),
                                &abortable,
                            )
                            .await?;

                        let pskb = signer.to_hex().unwrap(); //  todo exception
                        tprintln!(ctx, "{pskb}");
                    }
                    "unlock" => {
                        if argv.len() != 1 {
                            return self.display_help(ctx, argv).await;
                        }

                        // Get locked UTXO set.
                        let spend_utxos: Vec<kaspa_rpc_core::RpcUtxosByAddressesEntry> = ctx.wallet().rpc_api().get_utxos_by_addresses(vec![script_p2sh.clone()]).await?;
                        let priority_fee_sompi = try_parse_optional_kaspa_as_sompi_i64(argv.first())?.unwrap_or(0) as u64;

                        if spend_utxos.is_empty() {
                            twarnln!(ctx, "No locked UTXO set found.");
                            return Ok(());
                        }

                        let references: Vec<(UtxoEntry, TransactionOutpoint)> =
                            spend_utxos.iter().map(|entry| (entry.utxo_entry.clone(), entry.outpoint)).collect();

                        let total_locked_sompi: u64 = spend_utxos.iter().map(|entry| entry.utxo_entry.amount).sum();

                        tprintln!(
                            ctx,
                            "{} locked UTXO{} found with total amount of {} Kaspa",
                            spend_utxos.len(),
                            if spend_utxos.is_empty() { "" } else { "s" },
                            sompi_to_kaspa(total_locked_sompi)
                        );

                        // Sweep UTXO set.
                        match unlock_utxos_fee_per_transaction(references, &receive_address, script_sig, priority_fee_sompi as u64) {
                            Ok(pskb) => {
                                let pskb_hex = pskb.to_hex().unwrap();
                                tprintln!(ctx, "{pskb_hex}");
                            }
                            Err(e) => tprintln!(ctx, "Error generating unlock PSKB: {}", e.to_string()),
                        }
                    }
                    "sign" => {
                        let bundle = match Bundle::try_from(payload) {
                            Ok(bundle) => bundle,
                            Err(e) => {
                                terrorln!(ctx, "Error while parsing bundle{}", e.to_string());
                                return Err(e.into());
                            }
                        };

                        // Sign PSKB using the account's receiver address.
                        match account.pskb_sign(&bundle, wallet_secret.clone(), payment_secret.clone(), Some(&receive_address)).await {
                            Ok(signed_pskb) => {
                                let pskb_pack = String::try_from(signed_pskb).unwrap(); // todo exception
                                tprintln!(ctx, "{pskb_pack}");
                            }
                            Err(e) => terrorln!(ctx, "{}", e.to_string()),
                        }
                    }
                    "address" => {
                        tprintln!(ctx, "\r\nP2SH address: {}", script_p2sh);
                    }
                    v => {
                        terrorln!(ctx, "unknown command: '{v}'\r\n");
                        return self.display_help(ctx, argv).await;
                    }
                }
            }
            "sign" => {
                if argv.len() != 1 {
                    return self.display_help(ctx, argv).await;
                } else {
                    let (wallet_secret, payment_secret) = ctx.ask_wallet_secret(None).await?;

                    let bundle_raw = argv.first().unwrap().as_str();
                    let bundle = match Bundle::try_from(bundle_raw) {
                        Ok(bundle) => bundle,
                        Err(e) => {
                            terrorln!(ctx, "Error while parsing bundle{}", e.to_string());
                            return Err(e.into());
                        }
                    };

                    match account.pskb_sign(&bundle, wallet_secret.clone(), payment_secret.clone(), None).await {
                        Ok(signed_pskb) => {
                            let pskb_pack = String::try_from(signed_pskb).unwrap();
                            tprintln!(ctx, "{pskb_pack}");
                        }
                        Err(e) => terrorln!(ctx, "{}", e.to_string()),
                    }
                }
            }
            "send" => {
                if argv.len() != 1 {
                    return self.display_help(ctx, argv).await;
                } else {
                    let bundle_raw = argv.first().unwrap().as_str();
                    let bundle = match Bundle::try_from(bundle_raw) {
                        Ok(bundle) => bundle,
                        Err(e) => {
                            terrorln!(ctx, "Error while parsing bundle{ }", e.to_string());
                            return Err(e.into());
                        }
                    };
                    match account.pskb_broadcast(&bundle).await {
                        Ok(sent) => tprintln!(ctx, "Sent transactions {:?}", sent),
                        Err(e) => terrorln!(ctx, "Send error {:?}", e),
                    }
                }
            }
            "debug" => {
                let bundle_raw = argv.first().unwrap().as_str();
                let bundle = match Bundle::try_from(bundle_raw) {
                    Ok(bundle) => bundle,
                    Err(e) => {
                        terrorln!(ctx, "Error while parsing bundle{}", e.to_string());
                        return Err(e.into());
                    }
                };

                // Debug bundle view.
                tprintln!(ctx, "{:?}", bundle);
                let rust = bundle.inner_list.first().unwrap();

                let pskt: PSKT<Signer> = PSKT::<Signer>::from(rust.to_owned());
                let mut fin = pskt.finalizer();

                fin = finalize_pskt_one_or_more_sig_and_redeem_script(fin).expect("Finalized PSKT");

                // Verify if extraction is possible.
                match fin.extractor() {
                    Ok(ex) => match ex.extract_tx() {
                        Ok(_) => tprintln!(ctx, "Transaction extracted successfuly"),
                        Err(e) => terrorln!(ctx, "Extraction error {}", e.to_string()),
                    },
                    Err(_) => twarnln!(ctx, "PSKB not finalized"),
                }
            }
            v => {
                tprintln!(ctx, "unknown command: '{v}'\r\n");
                return self.display_help(ctx, argv).await;
            }
        }
        Ok(())
    }

    async fn display_help(self: Arc<Self>, ctx: Arc<KaspaCli>, _argv: Vec<String>) -> Result<()> {
        ctx.term().help(
            &[
                ("pskb create <address> <amount> <priority fee>", "Create a PSKB from single send transaction"),
                ("pskb sign <pskb>", "Sign given PSKB"),
                ("pskb send <pskb>", "Broadcast bundled transactions"),
                ("pskb debug <payload>", "Print PSKB debug view"),
                ("pskb script lock <payload> <amount> [priority fee]", "Generate a PSKB with one send transaction to given P2SH payload. Optional public key placeholder in payload: {{pubkey}}"),
                ("pskb script unlock <payload> <fee>", "Generate a PSKB to unlock UTXOS one by one from given P2SH payload. Fee amount will be applied to every spent UTXO, meaning every transaction. Optional public key placeholder in payload: {{pubkey}}"),
                ("pskb script sign <pskb>", "Sign all PSKB's P2SH locked inputs"),
                ("pskb script sign <pskb>", "Sign all PSKB's P2SH locked inputs"),
                ("pskb script address <pskb>", "Prints P2SH address"),
            ],
            None,
        )?;

        Ok(())
    }
}
