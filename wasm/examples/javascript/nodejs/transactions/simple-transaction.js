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
    serializeTransaction,
    deserializeTransaction,
    signTransaction,
    SignableTransaction,
    Transaction,
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

        // serialize tx 
        let serializedTransactions = transactions.map(tx=>tx.serialize());

        // test : deserialize tx and sign it
        let signedTransactions = serializedTransactions.map(tx_json=>{
            let signable_tx = SignableTransaction.deserialize(tx_json);
            return signTransaction(signable_tx, [privateKey], true)
        });

        //submit tx
        let result = await Promise.all(signedTransactions.map(async(transaction)=>{
            return await rpc.submitTransaction({transaction});
        }));

        console.log("result", result)

    }

    await rpc.disconnect();

})();