import http from 'http';
import fs from 'fs';

export function getSessionId(req){
    const cookies = req.headers.cookie;
    let sessionId;

    if (cookies) {
        const cookieParts = cookies.split(';').map(cookie => cookie.trim().split('='));
        const sessionCookie = cookieParts.find(cookie => cookie[0] === 'sessionId');

        if (sessionCookie) {
            sessionId = sessionCookie[1];
        }
    }
    return sessionId
}

export function generateSessionId() {
    return Math.random().toString(36).substring(2, 15);
}

export class SessionManager{
    constructor(config={file:"session.json"}){
        this.config = config
        this.data = {};
        this.load();
    }

    load(){
        fs.stat(this.config.file, (err)=>{
            if (!err){
                this.data = JSON.parse(fs.readFileSync(this.config.file)+"");
            }
        });
    }

    save(){
        fs.writeFileSync(this.config.file, JSON.stringify(this.data, null, "\t"));
    }

    /**
     * @param {http.IncomingMessage} req
     * @returns {{address: string; username: string;}|undefined} Returns a Object or undefined.
     */
    getByReq(req){
        let sessionId = getSessionId(req);
        return this.get(sessionId)
    }

    /**
     * @param {string} sessionId
     * @returns {?{address: string; username: string;}} Returns a Object or undefined.
     */
    get(sessionId){
        if (sessionId && this.data[sessionId]) {
            return this.data[sessionId]
        }
        return undefined
    }

    /**
     * @param {string} sessionId
     * @param {?{address: string; username: string;}} data
     */
    set(sessionId, data){
        this.data[sessionId] = data;
        this.save();
    }

    /**
     * @param {http.IncomingMessage} req
     */
    deleteByReq(req){
        let sessionId = getSessionId(req);
        this.delete(sessionId)
    }

    /**
     * @param {string} sessionId
     */
    delete(sessionId){
        if (sessionId && this.data[sessionId]){
            delete this.data[sessionId];
            this.save()
        }
    }

}