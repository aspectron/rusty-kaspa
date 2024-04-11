// TODO - NodeJs HTTP server with Kaspa Wallet and a client-facing WebSocket (example backend that receives payments)

import path from 'path';
import fs from 'fs';
import { Mnemonic, XPrv } from "../../nodejs/kaspa/kaspa.js";
import { SessionManager, generateSessionId } from "./session.js";
import { CacheManager } from "./cache.js";
import { HttpServer } from "./http-server.js";

/**
 * @param {string} networkId
 * @param {Mnemonic} mnemonic
 */
function createAccount(networkId, mnemonic) {
    //console.log("mnemonic:", mnemonic.phrase);
    let xprv = new XPrv(mnemonic.toSeed());
    let account_0_root = xprv.derivePath("m/44'/111111'/0'").toXPub();
    let account_0 = {
        receive_xpub : account_0_root.deriveChild(0),
        change_xpub : account_0_root.deriveChild(1),
    };
    return {
        account_0,
        networkId,
        receiveAddress(index=0){
            return this.account_0.receive_xpub.deriveChild(index).toPublicKey().toAddress(this.networkId).toString();
        },
        changeAddress(index=0){
            return this.account_0.change_xpub.deriveChild(index).toPublicKey().toAddress(this.networkId).toString();
        },
    };
}

const configFile = path.join('../data/config.json');
let walletAccount = null;
if (fs.existsSync(configFile)) {
    let config = JSON.parse(fs.readFileSync(configFile, "utf8"));
    let mnemonic = new Mnemonic(config.mnemonic);
    walletAccount = createAccount(config.networkId, mnemonic);
}else{
    console.info("Please create a config file by running 'node init' in the 'examples/' folder")
    process.exit(1);
}


const port = process.argv[2] || "8000";
const sessionManager = new SessionManager();
const cacheManager = new CacheManager();

const server = new HttpServer();

server.setSessionHandler((res)=>{
    let userSession = sessionManager.getByReq(res.req);
    if (userSession)
        return userSession

    const sessionId = generateSessionId();
    let address_index = cacheManager.get("address_index", 0);
    
    sessionManager.set(sessionId, {
        username: "demo-user-"+sessionId,
        address: walletAccount.receiveAddress(address_index),
    });

    cacheManager.set("address_index", address_index+1);
    // Set session ID as a cookie
    res.setHeader('Set-Cookie', `sessionId=${sessionId}; path=/; HttpOnly`);
    res.writeHead(302, { 'Location': res.req.url });
    res.end();
    return
});

server.setApiHandler((endpoint, userSession, res)=>{
    res.setHeader('Content-Type', 'text/plain');
    //console.log("req", req)
    switch (endpoint){
        case "address":{
            res.write(JSON.stringify({address: userSession.address}));
        }break;
        case "check-payment":{
            if (!userSession){
                res.write("please login first");
            }else{
                res.write("TODO");
            }
        }break;
        default:{
            res.writeHead(500);
            res.write("Invalid API method");
        }
    }
    res.end();
});

server.listen(port);
