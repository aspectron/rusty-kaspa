// TODO - NodeJs HTTP server with Kaspa Wallet and a client-facing WebSocket (example backend that receives payments)


const http = require('http');
const {URL} = require('url');
const path = require('path');
const fs = require('fs');
const qs = require('querystring');

const port = process.argv[2] || "8000";

const USERS = {
    xyz: "xyz",
    abc:"abc"
}
let sessions = {};

// Colors for CLI output
const WHT = '\x1B[39m';
const RED = "\x1B[91m";
const GRN = "\x1B[32m";

readSession();

http.createServer(function (req, res) {
    //let userSession = getUserSession(req);
    let userSession = ensureUserSession(req, res);
    if (!userSession){
        return
    }


    let url = req.url == "/" ? "/index.html": req.url;

    let pathname = new URL(url, "http://localhost").pathname;

    if (pathname.startsWith("/api/")){
        handleApiRequest(pathname.replace("/api/", ""), userSession, req, res)
        return
    }

    if (pathname == "/logout"){
        deleteUserSession(req);
        return redirectTo(res, "/")
    }

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

    function send(pathname, data={}){
        if (userSession){
            data.username = userSession.username;
            data.user_greeting_msg = "Hello: "+data.username;
        }
        sendFile(pathname, data, req, res);
    }

}).listen(parseInt(port, 10));


function generateSessionId() {
    return Math.random().toString(36).substring(2, 15);
}

function redirectTo(res, loc) {
    res.writeHead(302, { 'Location': loc  });
    res.end();
}

function saveSession(){
    fs.writeFileSync("session.json", JSON.stringify(sessions, null, "\t"));
}
function readSession(){
    fs.stat("session.json", (err, )=>{
        if (!err){
            sessions = JSON.parse(fs.readFileSync("session.json")+"");
        }
    });
    
}

function getSessionId(req){
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

function getUserSession(req){
    let sessionId = getSessionId(req);

    if (sessionId && sessions[sessionId] && sessions[sessionId].loggedIn) {
        return sessions[sessionId]
    }
    return false
}

function ensureUserSession(req, res){
    let userSession = getUserSession(req);
    if (userSession)
        return userSession

    const sessionId = generateSessionId();
    
    sessions[sessionId] = {
        username: "demo-user-"+sessionId,
        loggedIn: true
    };

    saveSession()

    // Set session ID as a cookie
    res.setHeader('Set-Cookie', `sessionId=${sessionId}; HttpOnly`);
    res.writeHead(302, { 'Location': '/' });
    res.end();
    return
}

function deleteUserSession(req){
    let sessionId = getSessionId(req);
    if (sessionId && sessions[sessionId]){
        delete sessions[sessionId];
        saveSession()
    }
}

function sendFile(pathname, data, req, res){
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
            console.log(GRN + 'FLDR: ' + WHT + pathname);
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
                content = content.replace(/<IF-USER>([\s\S]+?)<\/IF-USER>/g, (match, code) => {
                    return code
                });
                content = content.replace(/<IF-NOT-USER>([\s\S]+?)<\/IF-NOT-USER>/g, (match, code) => {
                    return ""
                });
            }else{
                content = content.replace(/<IF-USER>([\s\S]+?)<\/IF-USER>/g, (match, code) => {
                    return ""
                });
                content = content.replace(/<IF-NOT-USER>([\s\S]+?)<\/IF-NOT-USER>/g, (match, code) => {
                    return code
                });
            }
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

function handleApiRequest(endpoint, userSession, req, res){
    res.setHeader('Content-Type', 'text/plain');
    //console.log("req", req)
    switch (endpoint){
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
}

// Message to display when server is started
console.log(WHT + 'Static file server running at\n  => http://localhost:' + port + '/\nCTRL + C to shutdown');