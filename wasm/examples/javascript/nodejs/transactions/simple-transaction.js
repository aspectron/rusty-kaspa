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
    varifyTransactionSignature,
    PendingTransaction,
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
        entries.sort((a, b) => a.utxoEntry.amount > b.utxoEntry.amount ? -1 : 1);

        let { transactions, summary } = await createTransactions({
            entries,
            outputs: [{ address : destinationAddress, amount : kaspaToSompi(1)}],
            priorityFee: 0n,
            changeAddress: sourceAddress,
            networkId,
        });

        //console.log("Summary:", summary);
        //console.log("transactions", transactions[0])

        // let result = await Promise.all(transactions.map(async(tx)=>{
        //     tx.sign([privateKey]);
        //     return tx.submit(rpc);
        // }));


        let signedTransactions2 = [];
        // serialize tx 
        let serializedTransactions = transactions.map(tx=>{
            console.log("entries111", tx.getUtxoEntries())
            let json = tx.serialize();
            tx.sign([privateKey]);
            signedTransactions2.push(tx.serialize())
            return json
        });
        console.log("serializedTransactions.length", serializedTransactions.length);

        // test : deserialize tx and sign it
        let signedTransactions = serializedTransactions.map(tx_json=>{
            let signable_tx = SignableTransaction.deserialize(tx_json);
            console.log("entries2222", signable_tx.entries)
            return signTransaction(signable_tx, [privateKey], false)
        });

        let json1 = signedTransactions2[0];
        let json2 = signedTransactions[0].serialize();
        console.log("signedTransactions[0]", json1);
        console.log("signedTransactions[0]", json2);
        let result;
        if (json1 != json2){
            console.error("mismatch");
        }else{
            result = await rpc.submitTransaction({transaction: signedTransactions[0]});
        }

        // let signedTransaction = SignableTransaction.deserialize('{"entries":[{"address":"kaspatest:qpa8gs8w0quc3ghpx2l2dv30ny0mjuwyaj30xduw92v6mmta7df6uuz3ryfhy","amount":599899592381,"block_daa_score":33596456,"index":1,"is_coinbase":false,"script_public_key":"0000207a7440ee783988a2e132bea6b22f991fb971c4eca2f3378e2a99aded7df353aeac","transaction_id":"509f8dfcc9d54aa4786fdbb4929e0845b2b5013de1573a6364fa15939baf9c38"},{"address":"kaspatest:qpa8gs8w0quc3ghpx2l2dv30ny0mjuwyaj30xduw92v6mmta7df6uuz3ryfhy","amount":100000000,"block_daa_score":33596456,"index":0,"is_coinbase":false,"script_public_key":"0000207a7440ee783988a2e132bea6b22f991fb971c4eca2f3378e2a99aded7df353aeac","transaction_id":"509f8dfcc9d54aa4786fdbb4929e0845b2b5013de1573a6364fa15939baf9c38"}],"tx":{"gas":0,"id":"aa8b74d1f3d7a4c87eb7c67cfd0e35a46e18da9585d1464f0bcb8a2bb384eff1","inputs":[{"previousOutpoint":{"index":0,"transactionId":"509f8dfcc9d54aa4786fdbb4929e0845b2b5013de1573a6364fa15939baf9c38"},"sequence":0,"sigOpCount":1,"signatureScript":"41d23e9bc5fcdb3ca98ec9af5ee580d96e58d52102797ec91bd7c17bb905035b857aad80702aac0f49fcaa9315dd935f8bb1cd2ac1a183638ad2699df1c58502c001"},{"previousOutpoint":{"index":1,"transactionId":"509f8dfcc9d54aa4786fdbb4929e0845b2b5013de1573a6364fa15939baf9c38"},"sequence":0,"sigOpCount":1,"signatureScript":"410becaaad69bb50e65dba44e87993f34bff4ba84ed1dec86e0afb75a7f984ff4896aefcbc4d4580c0f6063d83f7dc027fda4100b6d10bd6e9d01551ac81c6ceeb01"}],"lockTime":0,"mass":0,"outputs":[{"scriptPublicKey":"0000207a7440ee783988a2e132bea6b22f991fb971c4eca2f3378e2a99aded7df353aeac","value":100000000},{"scriptPublicKey":"0000207a7440ee783988a2e132bea6b22f991fb971c4eca2f3378e2a99aded7df353aeac","value":599899579232}],"payload":"","subnetworkId":"0000000000000000000000000000000000000000","version":0}}');
        // signedTransaction = varifyTransactionSignature(signedTransaction);
        // signedTransactions = [signedTransaction];

        //submit tx
        // let result = await Promise.all(signedTransactions.map(async(transaction)=>{
            
        //     let tx = transaction.serialize();
        //     console.log("transaction", transaction, tx)
        //     return await rpc.submitTransaction({transaction});
        // }));

        console.log("result", result)

    }

    await rpc.disconnect();

})();