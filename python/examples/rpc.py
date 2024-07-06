import asyncio
import json
import time

from kaspa import RpcClient


def subscription_callback(event, callback_id, **kwargs):
    print(kwargs.get('kwarg1'))
    print(f'{callback_id} | {event}')

async def rpc_subscriptions(client):
    client.add_event_listener('all', subscription_callback, callback_id=1, kwarg1='Im a kwarg!!')

    await client.subscribe_daa_score()
    await client.subscribe_virtual_chain_changed(True)

    await asyncio.sleep(10)

    await client.unsubscribe_daa_score()
    await client.unsubscribe_virtual_chain_changed(True)

async def rpc_calls(client):
    get_server_info_response = await client.get_server_info()
    print(get_server_info_response)

    block_dag_info_response = await client.get_block_dag_info()
    print(block_dag_info_response)

    tip_hash = block_dag_info_response['tipHashes'][0]
    get_block_request = {'hash': tip_hash, 'includeTransactions': True}
    get_block_response = await client.get_block_call(get_block_request)
    print(get_block_response)

    get_balances_by_addresses_request = {'addresses': ['kaspa:qqxn4k5dchwk3m207cmh9ewagzlwwvfesngkc8l90tj44mufcgmujpav8hakt', 'kaspa:qr5ekyld6j4zn0ngennj9nx5gpt3254fzs77ygh6zzkvyy8scmp97de4ln8v5']}
    get_balances_by_addresses_response =  await client.get_balances_by_addresses_call(get_balances_by_addresses_request)
    print(get_balances_by_addresses_response)

async def main():
    client = RpcClient(url = "ws://localhost:17110")
    await client.connect()
    print(f'Client is connected: {client.is_connected()}')

    await rpc_calls(client)
    await rpc_subscriptions(client)


if __name__ == "__main__":
    asyncio.run(main())