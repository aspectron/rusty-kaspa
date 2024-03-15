// TODO - NodeJs HTTP server with Kaspa Wallet and a client-facing WebSocket (example backend that receives payments)


const http = require('http');
const {URL} = require('url');
const path = require('path');
const fs = require('fs');

const port = process.argv[2] || "8000";

// Colors for CLI output
const WHT = '\x1B[39m';
const RED = "\x1B[91m";
const GRN = "\x1B[32m";


http.createServer(function (req, res) {
    let url = req.url == "/" ? "/index.html": req.url;

    let pathname = new URL(url, "http://localhost").pathname;

    if (pathname.startsWith("/api/")){
        handleApiRequest(pathname.replace("/api/", ""), req, res)
        return
    }

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
        fs.readFile(filename, 'binary', function (err, file) {
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

            // Output the read file to the browser for it to load
            res.writeHead(200);
            res.write(file, 'binary');
            res.end();
        });

    });

}).listen(parseInt(port, 10));

function handleApiRequest(endpoint, req, res){
    res.setHeader('Content-Type', 'text/plain');
    switch (endpoint){
        case "check-payment":{
            res.write("TODO");
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