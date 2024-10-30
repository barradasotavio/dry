window.api = new Proxy({}, {
    get: function (target, name) {
        return function () {
            return new Promise((resolve, reject) => {
                const call_id = Math.random().toString(36).slice(2, 11);
                const args = Array.from(arguments);
                const message = JSON.stringify({
                    call_id: call_id,
                    function: name,
                    arguments: args
                });
                window.ipcStore = window.ipcStore || {};
                window.ipcStore[call_id] = { resolve, reject };
                window.ipc.postMessage(message);
            });
        };
    }
})

window.ipcCallback = function (response) {
    const { call_id, result, error } = response;
    if (window.ipcStore && window.ipcStore[call_id]) {
        if (error) {
            window.ipcStore[call_id].reject(new Error(error));
        } else {
            window.ipcStore[call_id].resolve(result);
        }
        delete window.ipcStore[call_id];
    }
}