// @ts-ignore
globalThis.WebSocket = require('websocket').w3cwebsocket; // W3C WebSocket module shim


const path = require('path');
const fs = require('fs');
const kaspa = require('../../../../nodejs/kaspa-dev');
const {
    Wallet, setDefaultStorageFolder,
    AccountKind, Resolver,
    sompiToKaspaString,
    Address,
    verifyMessage
} = kaspa;

let storageFolder = path.join(__dirname, '../../../data/wallets').normalize();
if (!fs.existsSync(storageFolder)) {
    fs.mkdirSync(storageFolder);
}

setDefaultStorageFolder(storageFolder);

(async()=>{
    //const filename = "wallet-394";
    const filename = "wallet-395";

    const balance = {};
    let wallet;

    const chalk = new ((await import('chalk')).Chalk)();

    function log_title(title){
        console.log(chalk.bold(chalk.green(`\n\n${title}`)))
    }

    try {
        
        const walletSecret = "abc";
        wallet = new Wallet({resident: false, networkId: "testnet-11", resolver: new Resolver()});
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

        // Connect to rpc
        await wallet.connect();

        // Start wallet processing
        await wallet.start();
        let accounts;
        let firstAccount = {};
        async function listAccount(){
            // List accounts
            accounts = await wallet.accountsEnumerate({});
            firstAccount = accounts.accountDescriptors[0];

            //console.log("firstAccount:", firstAccount);

            // Activate Account
            await wallet.accountsActivate({
                accountIds:[firstAccount.accountId]
            });

            // log_title("Accounts");
            // accounts.accountDescriptors.forEach(a=>{
            //     console.log(`Account: ${a.accountId}`);
            //     console.log(`   Account type: ${a.kind.toString()}`);
            //     console.log(`   Account Name: ${a.accountName}`);
            //     console.log(`   Receive Address: ${a.receiveAddress}`);
            //     console.log(`   Change Address: ${a.changeAddress}`);
            //     console.log("")
            // });
        }

        await listAccount();

        log_title("Message Signing");

        const message = "Hello Kaspa!";
        const signResponse = await wallet.accountsSignMessage({
            accountId: firstAccount.accountId,
            message,
            walletSecret,
            noAuxRand: true //optional
            //address: firstAccount.receiveAddress //optional 
        });

        console.log("Sign response:", signResponse);

        const verified = await wallet.accountsVerifyMessage({
            accountId: firstAccount.accountId,
            message,
            signature: signResponse.signature,
            publicKey: signResponse.publicKey
            // should we support address? to verified instead of publicKey?
        });

        console.log("Verify response:", verified);

        // if (verifyMessage({message, signature: signResponse.signature, publicKey: signResponse.publicKey})) {
        //     console.info('Signature verified!');
        // } else {
        //     console.info('Signature is invalid!');
        // }
        
    } catch(ex) {
        console.error("Error:", ex);
    }
})();
