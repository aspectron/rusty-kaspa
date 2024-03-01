const fs = require('fs');
const path = require('path');
const nodeUtil = require('node:util');
const { parseArgs: nodeParseArgs, } = nodeUtil;

const {
    Address,
    Encoding,
    NetworkId,
    Mnemonic,
    XPrv,
    PublicKeyGenerator,
    createAddress,
    NetworkType,
} = require('../../../nodejs/kaspa');

/**
 * Helper function to parse command line arguments for running the scripts
 * @param options Additional options to configure the parsing, such as additional arguments for the script and additional help output to go with it
 * @returns Promise<{address: Address, change_address:Address, tokens: any, networkId: (NetworkId), encoding: (Encoding)}>
 */
function parseArgs(options = {
    additionalParseArgs: {},
    additionalHelpOutput: '',
}) {
    const script = path.basename(process.argv[1]);
    let args = process.argv.slice(2);
    const {
        values,
        positionals,
        tokens,
    } = nodeParseArgs({
        args,
        options: {
            ...options.additionalParseArgs,
            help: {
                type: 'boolean',
            },
            json: {
                type: 'boolean',
            },
            address: {
                type: 'string',
            },
            network: {
                type: 'string',
            },
            encoding: {
                type: 'string',
            },
            walletinfo:{
                type: 'boolean'
            },
            legacy:{
                type: 'boolean'
            }
        },
        tokens: true,
        allowPositionals: true
    });
    if (values.help) {
        console.log(`Usage: node ${script} [address] [mainnet|testnet-10|testnet-11] [--address <address>] [--network <mainnet|testnet-10|testnet-11>] [--encoding <borsh|json>] ${options.additionalHelpOutput}`);
        process.exit(0);
    }

    let config = null;
    
    const filename = `${values.legacy? 'legacy-':''}config.json`;

    let configFile = path.join(__dirname, `../../data/${filename}`);
    if (fs.existsSync(configFile)) {
        config = JSON.parse(fs.readFileSync(configFile, "utf8"));
    } else {
        console.error("Please create a config file by running 'node init' in the 'examples/' folder");
        process.exit(0);
    }
    if (values.walletinfo){
        console.log(`Config: ${configFile}`);
    }

    const addressRegex = new RegExp(/(kaspa|kaspatest):\S+/i);
    const addressArg = values.address ?? positionals.find((positional) => addressRegex.test(positional)) ?? null;
    let address = addressArg === null ? null : new Address(addressArg);

    const networkArg = values.network ?? positionals.find((positional) => positional.match(/^(testnet|mainnet|simnet|devnet)-\d+$/)) ?? config.networkId ?? null;
    if (!networkArg) {
        console.error('Network id must be specified: --network=(mainnet|testnet-<number>)');
        process.exit(1);
    }

    const networkId = new NetworkId(networkArg);
    let change_address = address;
    if (!addressArg){
        let mnemonic = new Mnemonic(config.mnemonic);
        let wallet = basicWallet(networkId, mnemonic);
        if (values.walletinfo){
            console.log("wallet", wallet);
        }
        address = new Address(wallet.receive_address);
        change_address = new Address(wallet.change_address);
    }

    const encodingArg = values.encoding ?? positionals.find((positional) => positional.match(/^(borsh|json)$/)) ?? null;
    let encoding = Encoding.Borsh;
    if (encodingArg == "json") {
        encoding = Encoding.SerdeJson;
    }

    return {
        address,
        change_address,
        networkId,
        encoding,
        tokens,
    };
}

/**
 * @param {string | NetworkId | NetworkType} networkId
 * @param {Mnemonic} mnemonic
 */
function basicWallet(networkId, mnemonic) {
    //console.log("mnemonic:", mnemonic.phrase);
    let xprv = new XPrv(mnemonic.toSeed());
    let account_0_root = xprv.derivePath("m/44'/111111'/0'").toXPub();
    let account_0 = {
        receive_xpub : account_0_root.deriveChild(0),
        change_xpub : account_0_root.deriveChild(1),
    };
    let receive = account_0.receive_xpub.deriveChild(0).toPublicKey().toAddress(networkId).toString();
    // console.log("receive", receive)
    let change = account_0.change_xpub.deriveChild(0).toPublicKey().toAddress(networkId).toString();
    // console.log("change", change)

    // let keygen = PublicKeyGenerator.fromMasterXPrv(
    //     xprv.toString(),
    //     false,
    //     0n,0
    // );
    // let receive_addreses = keygen.receivePubkeys(0,1).map(key => key.toAddress(networkId).toString());
    // let change_addreses = keygen.changePubkeys(0,1).map(key => key.toAddress(networkId).toString());

    // console.log("receive_addreses:", receive_addreses);
    // console.log("change_addreses:", change_addreses);

    return {
        mnemonic: mnemonic.phrase,
        xprv: xprv.toString(),
        receive_address: receive,
        change_address: change,
    };
}

module.exports = {
    parseArgs,
};
