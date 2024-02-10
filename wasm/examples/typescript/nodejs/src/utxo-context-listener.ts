// Run with: node demo.js
import {w3cwebsocket} from "websocket";
(globalThis.WebSocket as any) = w3cwebsocket;

import {
    PrivateKey,
    Address,
    RpcClient,
    UtxoProcessor,
    UtxoContext,
    kaspaToSompi,
    createTransactions,
    initConsolePanicHook
} from "../../../../nodejs/kaspa/kaspa_wasm";

import {parseArgs} from "./utils";


initConsolePanicHook();


let { encoding, networkId, destinationAddress } = parseArgs();

(async () => {

    const privateKey = new PrivateKey('b99d75736a0fd0ae2da658959813d680474f5a740a9c970a7da867141596178f');
    const sourceAddress = privateKey.toKeypair().toAddress(networkId);
    let address = new Address("kaspa:qrxkessyzxkv5ve7rj7u36nxxvtt08lsknnd8e8zw3p7xsf8ck0cqeuyrhkp0")
    address = new Address("kaspa:qpamkvhgh0kzx50gwvvp5xs8ktmqutcy3dfs9dc3w7lm9rq0zs76vf959mmrp");
    console.info(`Source address: ${sourceAddress}`);
    console.info(`address: ${address}`);

    // if not destination is specified, send back to ourselves
    destinationAddress = destinationAddress ?? sourceAddress;
    console.info(`Destination address: ${destinationAddress}`);

    // 1) Initialize RPC
    const rpc = new RpcClient("127.0.0.1", encoding, networkId);

    // 2) Create UtxoProcessor, passing RPC to it
    let processor = await new UtxoProcessor({ rpc, networkId });

    // 3) Create one of more UtxoContext, passing UtxoProcessor to it
    // you can create UtxoContext objects as needed to monitor different
    // address sets.
    let context = await new UtxoContext({ processor });

    // 4) Register a listener with the UtxoProcessor::events
    processor.events.registerListener((event:any) => {
        console.log("event:", event);
    });

    console.log(processor);

    // 5) Once the environment is setup, connect to RPC
    console.log(`Connecting to ${rpc.url}`);
    await rpc.connect(undefined);
    let { isSynced } = await rpc.getServerInfo();
    if (!isSynced) {
        console.error("Please wait for the node to sync");
        rpc.disconnect();
        return;
    }

    // 6) Register the address list with the UtxoContext
    await context.trackAddresses([sourceAddress, address], undefined);

})();