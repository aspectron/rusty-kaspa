// Run with: node demo.js
globalThis.WebSocket = require("websocket").w3cwebsocket;

const {
    PrivateKey,
    Address,
    RpcClient,
    Encoding,
    NetworkType,
    init_console_panic_hook
} = require('./kaspa/kaspa_wasm');

init_console_panic_hook();

async function runDemo() {
    // From BIP0340
    const sk = new PrivateKey('B7E151628AED2A6ABF7158809CF4F3C762E7160F38B4DA56A784D9045190CFEF');

    const kaspaAddress = sk.toKeypair().toAddress(NetworkType.Mainnet).toString();
    // Full kaspa address: kaspa:qr0lr4ml9fn3chekrqmjdkergxl93l4wrk3dankcgvjq776s9wn9jkdskewva
    console.info(`Full kaspa address: ${kaspaAddress}`);

    const addr = new Address(kaspaAddress);
    console.info(addr);

    console.info(sk.toKeypair().xOnlyPublicKey); // dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659
    console.info(sk.toKeypair().publicKey);      // 02dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659

    const rpcUrl = "ws://127.0.0.1:17110";
    const rpc = new RpcClient(Encoding.Borsh, rpcUrl);

    await rpc.connect();

    let utxos_by_address = await rpc.getUtxosByAddresses({ addresses: [addr.toString()] });

    console.info(utxos_by_address);
}

runDemo();