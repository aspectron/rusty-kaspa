import http from 'http';
import path from 'path';
import fs from 'fs';
import {URL} from 'url';
//import qs from 'querystring';

// Colors for CLI output
export const WHT = '\x1B[39m';
export const RED = "\x1B[91m";
export const GRN = "\x1B[32m";

/**
 * @param {string} pathname
 * @param {{ [x: string]: any; username?: string; }} data
 * @param {http.ServerResponse<http.IncomingMessage> & { req: http.IncomingMessage; }} res
 */
// @ts-ignore
function sendFile(pathname, data, res){
    // get the /file.html from above and then find it from the current folder
    let filename = path.join(process.cwd(), pathname);

    // Setting up MIME-Type
    let contentTypesByExtension = {
        '.html': 'text/html',
        '.css':  'text/css',
        '.js':   'text/javascript',
        '.json': 'text/json',
        '.svg':  'image/svg+xml'
    };

    // Check if the requested file exists
    fs.stat(filename, function (error, stat) {
        // If it doesn't
        if (error) {
            // Output a red error pointing to failed request
            console.log(RED + 'FAIL: ' + pathname);
            // Redirect the browser to the 404 page
            filename = path.join(process.cwd(), '/404.html');
        // If the requested URL is a folder, like http://localhost:8000/folder
        } else if (stat.isDirectory()) {
            // Output a green line to the console explaining what folder was requested
            console.log(GRN + 'FOLDER: ' + WHT + pathname);
            // redirect the user to the index.html in the requested folder
            filename += '/index.html';
        }

        // Assuming the file exists, read it
        fs.readFile(filename, 'utf8', function (err, content) {
            // Output a green line to console explaining the file that will be loaded in the browser
            console.log(GRN + 'FILE: ' + WHT + pathname);
            // If there was an error trying to read the file
            if (err) {
                // Put the error in the browser
                res.writeHead(500, {'Content-Type': 'text/plain'});
                res.write(err + '\n');
                res.end();
                return;
            }

            let contentType = contentTypesByExtension[path.extname(filename)];
            if (contentType) {
                res.setHeader('Content-Type', contentType);
            }

            if (data.username){
                // @ts-ignore
                content = content.replace(/<IF-USER>([\s\S]+?)<\/IF-USER>/g, (match, code) => {
                    return code
                });
                // @ts-ignore
                content = content.replace(/<IF-NOT-USER>([\s\S]+?)<\/IF-NOT-USER>/g, (match, code) => {
                    return ""
                });
            }else{
                // @ts-ignore
                content = content.replace(/<IF-USER>([\s\S]+?)<\/IF-USER>/g, (match, code) => {
                    return ""
                });
                // @ts-ignore
                content = content.replace(/<IF-NOT-USER>([\s\S]+?)<\/IF-NOT-USER>/g, (match, code) => {
                    return code
                });
            }
            // @ts-ignore
            content = content.replace(/<%=([\s\S]+?)%>/g, (match, code) => {
                return data[code.trim()]??"";
            });

            // Output the read file to the browser for it to load
            res.writeHead(200);
            res.write(content);
            res.end();
        });

    });
}

const defaultConfig = {
    /**
     * @param {http.ServerResponse<http.IncomingMessage>} res
     * @returns {{[x: string]: any;}?}
     */
    ensureUserSession(res){
        return {}
    }
}

export class HttpServer{
   
    constructor(config=defaultConfig){
        this.config = config;
        this.apiHandler = (/** @type {string} */ pathname, /** @type {{ [x: string]: any; }} */ userSession, /** @type {http.ServerResponse<http.IncomingMessage>} */ res)=>{

        }
    }

    /**
     * @param {{ (res: http.ServerResponse<http.IncomingMessage> & {req: http.IncomingMessage;}): {[x: string]: any;}?; }} handler
     */

    setSessionHandler(handler){
        this.sessionHandler = handler;
    }

    /**
     * @param {{ (endpoint: string, userSession: { address: string; }, res: http.ServerResponse<http.IncomingMessage> & { req: http.IncomingMessage; // Put the error in the browser
     }): void; }} handler
     */
    setApiHandler(handler){
        this.apiHandler = handler;
    }
    

    /**
     * @param {string|number} port
     */
    listen(port){
        let portNumber = port;
        if (typeof port == 'string'){
            portNumber = parseInt(port, 10);
        }

        http.createServer((req, res)=>{
            // @ts-ignore
            res.sendFile = (/** @type {string} */ pathname, /** @type {{ [x: string]: any; }} */ data)=>{
                sendFile(pathname, data, res)
            };
            // @ts-ignore
            res.redirectTo = (/** @type {string} */ loc)=>{
                res.writeHead(302, { 'Location': loc  });
                res.end();
            };
            
            //let userSession = getUserSession(req);
            let userSession = null;
            if (this.sessionHandler){
                userSession = this.sessionHandler(res);
                if (!userSession){
                    return
                }
            }
        
        
            let url = req.url == "/" ? "/index.html": req.url;
        
            let pathname = new URL(url, "http://localhost").pathname;
        
            if (pathname.startsWith("/api/")){
                this.apiHandler(pathname.replace("/api/", ""), userSession, res)
                return
            }
        
            // if (pathname == "/logout"){
            //     this.sessionManager.deleteByReq(req);
            //     // @ts-ignore
            //     return req.redirectTo(res, "/")
            // }
        
            // if (pathname == "/login.html" && req.method == "POST"){
            //     let body = "";
            //     req.on("data", chunk => {
            //         body += chunk.toString();
            //     });
        
            //     req.on('end', () => {
            //         const info = qs.parse(body);
            //         if (USERS[info.username] == info.password){
            //             const sessionId = generateSessionId();
        
            //             sessions[sessionId] = {
            //                 username: info.username,
            //                 loggedIn: true
            //             };
        
            //             saveSession()
        
            //             // Set session ID as a cookie
            //             res.setHeader('Set-Cookie', `sessionId=${sessionId}; HttpOnly`);
            //             res.writeHead(302, { 'Location': '/' });
            //             res.end();
            //             return
            //         }
        
            //         send("login.html", {error:"Invalid username or password"});
            //     })
                
            //     return
            // }
        
            // if (!userSession 
            //     && !pathname.startsWith("/index.html")
            //     && !pathname.startsWith("/resources")
            //     && !pathname.startsWith("/login")){
            //     return redirectTo(res, '/login.html')
            // }
        
            // update session data and save it
            // userSession.abc = "xyz";
            // saveSession()
        
            send(pathname);
        
            /**
             * @param {string} pathname
             */
            function send(pathname, data={}){
                if (userSession){
                    data.username = userSession.username;
                    data.address = userSession.address;
                    data.user_greeting_msg = "Hello: "+data.username;
                }
                // @ts-ignore
                res.sendFile(pathname, data);
            }
        
        }).listen(portNumber);

        // Message to display when server is started
        console.log(WHT + 'Server running at\n  => http://localhost:' + portNumber + '/\nCTRL + C to shutdown');
    }
}