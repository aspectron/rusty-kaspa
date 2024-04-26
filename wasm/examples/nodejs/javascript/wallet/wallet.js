// @ts-ignore
globalThis.WebSocket = require('websocket').w3cwebsocket; // W3C WebSocket module shim

const path = require('path');
const fs = require('fs');
const kaspa = require('../../../../nodejs/kaspa');
const {
    Wallet, setDefaultStorageFolder,
    AccountKind, Mnemonic, Resolver,
    kaspaToSompi,
    Address
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

        const balance = {};

        wallet.addEventListener(({type, data})=>{

            // if (type == "maturity"){
            //     console.log("record.hasAddress :receive:", data.hasAddress(firstAccount.receiveAddress));
            //     console.log("record.hasAddress :change:", data.hasAddress(firstAccount.changeAddress));
            //     console.log("record.data", data.data)
            //     console.log("record.blockDaaScore", data.blockDaaScore)
            // }
            if (type == "balance"){
                balance[data.id] = data.balance;
                console.log("balance updated:", balance);
                return
            }
            if (type == "daa-score-change"){
                if (data.currentDaaScore%1000 == 0){
                    console.log(`[${type}]:`, data.currentDaaScore)
                }
            }else{
                console.log(`[${type}]:`, data)
            }
        })

        // Open wallet
        await wallet.walletOpen({
            walletSecret,
            filename,
            accountDescriptors: false
        });

        // Ensure default account
        await wallet.accountsEnsureDefault({
            walletSecret,
            type: new AccountKind("bip32") // "bip32"
        });

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
        await wallet.start();

        // List accounts
        let accounts = await wallet.accountsEnumerate({});
        let firstAccount = accounts.accountDescriptors[0];

        //console.log("firstAccount:", firstAccount);

        // Activate Account
        await wallet.accountsActivate({
            accountIds:[firstAccount.accountId]
        });

        accounts.accountDescriptors.forEach(a=>{
            console.log(`\nAccount: ${a.accountId}`);
            console.log(`   Account type: ${a.kind.toString()}`);
            console.log(`   Account Name: ${a.accountName}`);
            console.log(`   Receive Address: ${a.receiveAddress}`);
            console.log(`   Change Address: ${a.changeAddress}`);
        });

        // Send a test transaction
        let sendResult = await wallet.accountsSend({
            walletSecret,
            accountId: firstAccount.accountId,
            priorityFeeSompi: kaspaToSompi("0.001"),
            destination:[{
                address: firstAccount.changeAddress,
                amount: kaspaToSompi("1")
            }]
        });
        console.log("sendResult", sendResult);
        
        
        
    } catch(ex) {
        console.error("Error:", ex);
    }
})();