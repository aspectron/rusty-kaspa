use crate::imports::*;
pub use kaspa_wallet_core::account::pskb::Bundle;

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
                    // let account = ctx.select_account().await?;
                    let amount_sompi = try_parse_required_nonzero_kaspa_as_sompi_u64(argv.get(1))?;
                    let outputs = PaymentOutputs::from((address.clone(), amount_sompi));
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
            "sign" => {
                if argv.len() != 1 {
                    return self.display_help(ctx, argv).await;
                } else {
                    let (wallet_secret, payment_secret) = ctx.ask_wallet_secret(None).await?;

                    let pskb = argv.first().unwrap().as_str();
                    let signed_pskb =
                        account.pskb_sign(&Bundle::try_from(pskb).unwrap(), wallet_secret.clone(), payment_secret.clone()).await?;
                    let pskb_hex = signed_pskb.to_hex().unwrap();

                    tprintln!(ctx, "{pskb_hex}");
                }
            }
            "send" => {
                if argv.len() != 1 {
                    return self.display_help(ctx, argv).await;
                } else {
                    let pskb: &str = argv.first().unwrap().as_str();

                    match account.pskb_broadcast(&Bundle::try_from(pskb).unwrap()).await {
                        Ok(sent) => tprintln!(ctx, "Sent transactions {:?}", sent),
                        Err(e) => tprintln!(ctx, "Send error {:?}", e),
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
                ("pskb create <address> <amount> <priority fee>", "Create a PSKB from single send transaction using current account"),
                ("pskb sign <pskb>", "Sign given PSKB in the context of current account"),
                ("pskb send <pskb>", "Broadcast bundled transactions"),
            ],
            None,
        )?;

        Ok(())
    }
}
