globalThis.WebSocket = require("websocket").w3cwebsocket;
// import { RpcClient, Resolver, PrivateKey, createTransactions, UtxoProcessor, UtxoContext } from "./wasm"
// import { RpcClient, Resolver, PrivateKey, createTransactions, UtxoProcessor, UtxoContext } from '../../../nodejs/kaspa-dev'
const { RpcClient, Resolver, PrivateKey, createTransactions, UtxoProcessor, UtxoContext } = require('../../../nodejs/kaspa-dev');

function defer() {
    let resolve, reject;
    const p = new Promise((resolve_, reject_) => {
        resolve = resolve_;
        reject = reject_;
    });
    p.resolve = resolve;
    p.reject = reject;
    return p;
}

let globals = {};

async function test() {

    const rpc = new RpcClient({
        resolver: new Resolver(),
        networkId: 'testnet-11'
    });

    globals.rpc = rpc;

    await rpc.connect()
    console.log('connected')
    const privateKey = new PrivateKey('46bc88d63599a2c14cafefcad4490a7222416d5e765f78f2de92fe6579c56bad')
    const publicKey = privateKey.toPublicKey()
    const address = publicKey.toAddress('testnet-11')

    const processor = new UtxoProcessor({
        rpc,
        networkId: 'testnet-11'
    })

    globals.processor = processor;

    const context = new UtxoContext({
        processor
    })

    globals.context = context;

    processor.addEventListener('utxo-proc-start', async () => {
        context.trackAddresses([ address ])
    console.log("sleeping...");
    await new Promise(resolve => setTimeout(resolve, 2500));
    console.log("creating transactions...");

        const { transactions } = await createTransactions({
            entries: context,
            outputs: [{
                address,
                amount: BigInt(1e8)
            }],
            changeAddress: address.toString(),
            priorityFee: 100000n
        });

        for (const transaction of transactions) {
            globals.transaction = transaction;
            console.log('handling tx');
            transaction.sign([ privateKey ]);
            console.log('signed');
            let rpc = processor.rpc;
            await transaction.submit(rpc);
            // await transaction.submit(rpc);
            // let ret = await transaction.submit(rpc);
            // console.log("done");
            // // await transaction.submit(processor.rpc);
            console.log('inner content', rpc);
            // console.log('inner content', transaction);
        }
    })

    await processor.start();
}

(async () => {
    await test();
    let p = defer();
    await p;
    console.log("FINISHING...");
})();

