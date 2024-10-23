window.api = new Proxy({}, {
    get: function (target, name) {
        return function () {
            return new Promise((resolve, reject) => {
                const callId = Math.random().toString(36).slice(2, 11);
                const args = Array.from(arguments).join(',');
                const message = `${callId}:${name},${args}`;
                window.ipcStore = window.ipcStore || {};
                window.ipcStore[callId] = { resolve, reject };
                window.ipc.postMessage(message);
            });
        };
    }
})

window.ipcCallback = function (response) {
    const { callId, result, error } = JSON.parse(response);
    if (window.ipcStore && window.ipcStore[callId]) {
        if (error) {
            window.ipcStore[callId].reject(new Error(error));
        } else {
            window.ipcStore[callId].resolve(result);
        }
        delete window.ipcStore[callId];
    }
}