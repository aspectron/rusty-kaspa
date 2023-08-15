globalThis.WebSocket = require("websocket").w3cwebsocket;

let kaspa = require('./kaspa/kaspa_wasm');
let { RpcClient, Encoding, Address } = kaspa;
kaspa.init_console_panic_hook();

let URL = "ws://127.0.0.1:17110";
let rpc = new RpcClient(Encoding.Borsh, URL);

let addresses = [
    new Address("kaspatest:qz7ulu4c25dh7fzec9zjyrmlhnkzrg4wmf89q7gzr3gfrsj3uz6xjceef60sd"),
];

rpc.getUtxosByAddresses({addresses});
