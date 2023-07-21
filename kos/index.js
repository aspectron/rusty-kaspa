true && process.versions["nw-flavor"] === "sdk" && chrome.developerPrivate.openDevTools({
	renderViewId: -1,
	renderProcessId: -1,
	extensionId: chrome.runtime.id,
});

window.open("/app/d3.html");
(async()=>{
    window.kaspa = await import('/app/wasm/kaspa.js');
    const wasm = await window.kaspa.default('/app/wasm/kaspa_bg.wasm');
    await window.kaspa.init_core();
})();
