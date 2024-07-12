use crate::imports::*;
use kaspa_consensus_core::tx::{TransactionOutpoint, UtxoEntry};
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
                // let keydata = account.prv_key_data(wallet_secret.clone()).await?;

                let script_sig = lock_script_sig(payload, Some(receive_address.payload_to_string())).unwrap(); //todo: Error
                let script_p2sh = script_addr(&script_sig, ctx.wallet().address_prefix()?).unwrap(); //todo: Error
                let script_public_key: kaspa_consensus_core::tx::ScriptPublicKey = script_public_key(&script_sig).unwrap(); //todo error

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

                        // Get locked UTXO set
                        let spend_utxos = ctx.wallet().rpc_api().get_utxos_by_addresses(vec![script_p2sh.clone()]).await?;
                        let priority_fee_sompi = try_parse_optional_kaspa_as_sompi_i64(argv.first())?.unwrap_or(0);

                        if spend_utxos.is_empty() {
                            return Ok(()); // todo Error
                        }

                        let references: Vec<(UtxoEntry, TransactionOutpoint)> =
                            spend_utxos.iter().map(|entry| (entry.utxo_entry.clone(), entry.outpoint)).collect();

                        match unlock_utxos(references, script_public_key, script_sig, priority_fee_sompi as u64) {
                            Ok(pskb) => {
                                let pskb_hex = pskb.to_hex().unwrap();
                                tprintln!(ctx, "{pskb_hex}");
                            }
                            Err(e) => tprintln!(ctx, "Error generating unlock PSKB: {}", e.to_string()),
                        }
                    }
                    "sign" => {
                        let bundle_raw = argv.first().unwrap().as_str();
                        let bundle = Bundle::try_from(bundle_raw).unwrap(); // todo error

                        // Sign PSKB using the account's receiver address.
                        match account.pskb_sign(&bundle, wallet_secret.clone(), payment_secret.clone(), Some(&receive_address)).await {
                            Ok(signed_pskb) => {
                                let pskb_pack = String::try_from(signed_pskb).unwrap(); // todo exception
                                tprintln!(ctx, "{pskb_pack}");
                            }
                            Err(e) => terrorln!(ctx, "{}", e.to_string()),
                        }
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
                    let bundle = Bundle::try_from(bundle_raw).unwrap(); // todo error

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
                    let bundle = Bundle::try_from(bundle_raw).unwrap(); // todo error

                    match account.pskb_broadcast(&bundle).await {
                        Ok(sent) => tprintln!(ctx, "Sent transactions {:?}", sent),
                        Err(e) => terrorln!(ctx, "Send error {:?}", e),
                    }
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
                ("pskb script lock <payload> <amount> [priority fee]", "Generate a PSKB with one send transaction to given P2SH payload. Optional public key placeholder in payload: {{pubkey}}"),
                ("pskb script unlock <payload> <fee>", "Generate a PSKB to unlock UTXOS one by one from given P2SH payload. Fee amount will be applied to every UTXO spent. Optional public key placeholder in payload: {{pubkey}}"),
                ("pskb script sign <pskb>", "Sign all PSKB's P2SH locked inputs"),
            ],
            None,
        )?;

        Ok(())
    }
}
