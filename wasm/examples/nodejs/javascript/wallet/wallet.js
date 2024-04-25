// @ts-ignore
globalThis.WebSocket = require('websocket').w3cwebsocket; // W3C WebSocket module shim

const path = require('path');
const fs = require('fs');
const kaspa = require('../../../../nodejs/kaspa');
const {
    Wallet, setDefaultStorageFolder,
    AccountKind, Mnemonic, Resolver
} = kaspa;

let storageFolder = path.join(__dirname, '../../../data/wallets').normalize();
if (!fs.existsSync(storageFolder)) {
    fs.mkdirSync(storageFolder);
}
setDefaultStorageFolder(storageFolder);

(async()=>{
    try {
        const filename = "wallet-394";
        const walletSecret = "abc";
        let wallet = new Wallet({resident: false, networkId: "testnet-11", resolver: new Resolver()});
        //console.log("wallet", wallet)
    
        // Ensure wallet file
        if (!await wallet.exists(filename)){
            let response = await wallet.walletCreate({
                walletSecret,
                filename,
                title: "W-1"
            });
            console.log("walletCreate : response", response)
        }

        wallet.addEventListener(({type, data})=>{
            console.log(`[${type}]:`, data)
        })

        // Open wallet file
        let res = await wallet.walletOpen({
            walletSecret,
            filename,
            accountDescriptors: true
        });

        //console.log("walletOpen: result", res)

        // Ensure default account
        let accountsEnsureDefaultRes = await wallet.accountsEnsureDefault({
            walletSecret,
            type: new AccountKind("bip32") // "bip32"
        });

        //console.log("accountsEnsure: result", accountsEnsureDefaultRes)

        // // Create a new account
        // // create private key
        // let prvKeyData =  await wallet.prvKeyDataCreate({
        //     walletSecret,
        //     mnemonic: Mnemonic.random(24).phrase
        // });

        // //console.log("prvKeyData", prvKeyData);

        // let account = await wallet.accountsCreate({
        //     walletSecret,
        //     type:"bip32",
        //     accountName:"Account-B",
        //     prvKeyDataId: prvKeyData.prvKeyDataId
        // });

        // console.log("new account:", account);

        // Connect to rpc
        await wallet.connect();

        // Start wallet processing
        // await wallet.start();

       

        // List accounts
        let accounts = await wallet.accountsEnumerate({});

        accounts.accountDescriptors.forEach(a=>{
            console.log(`\nAccount: ${a.accountId}`);
            console.log(`   Account type: ${a.kind.toString()}`);
            console.log(`   Account Name: ${a.accountName}`);
            console.log(`   Receive Address: ${a.receiveAddress}`);
            console.log(`   Change Address: ${a.changeAddress}`);
        })
        
        
    } catch(ex) {
        console.error("Error:", ex);
    }
})();