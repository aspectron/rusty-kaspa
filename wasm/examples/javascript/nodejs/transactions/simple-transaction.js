// Run with: node demo.js
// @ts-ignore
globalThis.WebSocket = require("websocket").w3cwebsocket;

const {
    PrivateKey,
    Address,
    RpcClient,
    kaspaToSompi,
    createTransactions,
    initConsolePanicHook,
    deserializeTransaction,
    signTransaction,
} = require('../../../../nodejs/kaspa');

const { encoding, networkId, address: destinationAddressArg } = require("../utils").parseArgs();

initConsolePanicHook();

(async () => {


    // From BIP0340
    const privateKey = new PrivateKey('b99d75736a0fd0ae2da658959813d680474f5a740a9c970a7da867141596178f');

    const sourceAddress = privateKey.toKeypair().toAddress(networkId);
    console.info(`Source address: ${sourceAddress}`);

    // if not destination address is supplied, send funds to source address
    const destinationAddress = destinationAddressArg || sourceAddress;
    console.log(`Destination address: ${destinationAddress}`);

    const rpc = new RpcClient({
        url : "wss://eu-1.kaspa-ng.org/testnet-11",
        encoding,
        networkId
    });
    console.log(`Connecting to ${rpc.url}`);

    await rpc.connect();
    let { isSynced, virtualDaaScore } = await rpc.getServerInfo();
    if (!isSynced) {
        console.error("Please wait for the node to sync");
        rpc.disconnect();
        return;
    }

    let { entries } = (await rpc.getUtxosByAddresses([sourceAddress]));

    if (!entries.length) {
        console.error("No UTXOs found for address");
    } else {
        console.info(entries);

        // a very basic JS-driven utxo entry sort
        entries.sort((a, b) => a.amount > b.amount ? 1 : -1);

        let { transactions, summary } = await createTransactions({
            entries,
            outputs: [{ address : destinationAddress, amount : kaspaToSompi(1)}],
            priorityFee: 0n,
            changeAddress: sourceAddress,
            networkId,
        });

        console.log("Summary:", summary);
        console.log("transactions", transactions[0])

        for (let pending of transactions) {
            let tx_json = pending.serializeJSON();
            console.log("Pending transaction serializeJSON:", tx_json);
            let signable_tx = await deserializeTransaction(tx_json);
            // console.log("Signing tx with secret key:", privateKey.toString());
            // await pending.sign([privateKey]);
            // console.log("Submitting pending tx to RPC ...")
            // let txid = await pending.submit(rpc);
            // console.log("Node responded with txid:", txid);

            const transaction = signTransaction(signable_tx, [privateKey], true);
            let result = await rpc.submitTransaction({transaction});

            console.info(result);
        }
    }

    await rpc.disconnect();

})();