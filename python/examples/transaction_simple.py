import asyncio
from kaspa import (
    Keypair,
    PrivateKey,
    RpcClient,
    Resolver,
    PaymentOutput,
    create_transaction,
    sign_transaction
)

async def main():
    private_key = PrivateKey("389840d7696e89c38856a066175e8e92697f0cf182b854c883237a50acaf1f69")
    source_address = private_key.to_keypair().to_address(network="kaspatest")
    print(source_address.to_string())
    destination_address = source_address

    client = RpcClient(resolver=Resolver(), network="testnet", network_suffix=10)
    await client.connect()
    print(f"Client is connected: {client.is_connected}")

    utxos = await client.get_utxos_by_addresses({"addresses": [source_address]})
    utxos = utxos["entries"]

    utxos = sorted(utxos, key=lambda x: x['utxoEntry']['amount'], reverse=True)
    total = sum(item['utxoEntry']['amount'] for item in utxos)

    fee_rates = await client.get_fee_estimate()
    fee = int(fee_rates["estimate"]["priorityBucket"]["feerate"])

    total_less_fee = total - fee
    utxo1_amt = int(total_less_fee * .1)
    utxo2_amt = int(total_less_fee * .9)

    outputs = [
        {"address": destination_address, "amount": utxo1_amt},
        {"address": destination_address, "amount": utxo2_amt},
    ]

    tx = create_transaction(utxos, outputs, 0, None, 1)
    tx_signed = sign_transaction(tx, [private_key], True)

    print(await client.submit_transaction({
        "transaction": tx_signed,
        "allow_orphan": True
    }))

if __name__ == "__main__":
    asyncio.run(main())