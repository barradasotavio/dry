// The Api proxy, exposed as dry.api. Every property read returns a function
// that posts a Call over the Bridge and resolves a Promise with Python's
// return value.
//
// The pending-call store lives in this closure, so no page script can read or
// tamper with in-flight Calls. Rust resolves them through dry.resolveCall,
// which is non-enumerable and non-writable: reachable from the evaluated
// script Rust sends, invisible to anything enumerating the namespace.

(() => {
    const pending = new Map();

    const api = new Proxy({}, {
        get: (_target, name) => (...args) => new Promise((resolve, reject) => {
            const call_id = Math.random().toString(36).slice(2, 11);
            pending.set(call_id, { resolve, reject });
            window.ipc.postMessage(JSON.stringify({
                call_id: call_id,
                function: name,
                arguments: args,
            }));
        }),
    });

    const resolveCall = (response) => {
        const { call_id, result, error } = response;
        const call = pending.get(call_id);
        if (!call) return;
        pending.delete(call_id);
        if (error) {
            // Rust sends "TypeName: message", so the Python exception's type
            // survives as the name of the Error the Promise rejects with.
            const separator = error.indexOf(': ');
            const rejection = new Error(separator === -1 ? error : error.slice(separator + 2));
            if (separator !== -1) rejection.name = error.slice(0, separator);
            call.reject(rejection);
        } else {
            call.resolve(result);
        }
    };

    Object.defineProperty(window.dry, 'api', {
        value: api,
        writable: false,
        configurable: false,
        enumerable: true,
    });

    Object.defineProperty(window.dry, 'resolveCall', {
        value: resolveCall,
        writable: false,
        configurable: false,
        enumerable: false,
    });
})();
