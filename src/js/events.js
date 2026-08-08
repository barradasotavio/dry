// The Event half of the Bridge, exposed as dry.on, dry.off and dry.emit.
//
// An Event is a name and a value, and it returns nothing: emit posts and
// returns, and a listener's return value is dropped. That is the whole
// difference from dry.api, which spends a call id and a Promise on getting an
// answer back.
//
// The listener register lives in this closure, so no page script can read or
// unhook another script's listeners. Rust delivers through dry.deliverEvent,
// which is non-enumerable and non-writable, exactly like dry.resolveCall:
// reachable from the script Rust evaluates, invisible to anything enumerating
// the namespace.

(() => {
    // Names starting with this belong to Dry's own window Events. Anyone may
    // listen for one; nobody but Dry may emit one.
    const RESERVED = 'window:';

    const listeners = new Map();

    const on = (name, listener) => {
        if (typeof name !== 'string' || !name) {
            throw new TypeError('An Event needs a name.');
        }
        if (typeof listener !== 'function') {
            throw new TypeError('An Event listener must be a function.');
        }
        const registered = listeners.get(name);
        if (registered) {
            registered.push(listener);
        } else {
            listeners.set(name, [listener]);
        }
        // The unsubscribe a component holds on to, so it does not have to keep
        // the name and the function around to take it off again.
        return () => off(name, listener);
    };

    const off = (name, listener) => {
        const registered = listeners.get(name);
        if (!registered) return;
        const at = registered.indexOf(listener);
        if (at === -1) return;
        registered.splice(at, 1);
        if (registered.length === 0) listeners.delete(name);
    };

    const emit = (name, value) => {
        if (typeof name !== 'string' || !name) {
            throw new TypeError('An Event needs a name.');
        }
        if (name.startsWith(RESERVED)) {
            throw new TypeError(
                `'${name}' is a reserved Event name: a name starting with ` +
                `'${RESERVED}' belongs to Dry's own window Events. Listen for ` +
                `it as much as you like, but emit under a name of your own.`
            );
        }
        window.ipc.postMessage(
            'dry_event:' + JSON.stringify({ name: name, value: value })
        );
    };

    // Every listener registered for the name gets the Event, in the order they
    // registered, and one that throws does not rob the rest of theirs.
    const deliverEvent = (event) => {
        const registered = listeners.get(event.name);
        if (!registered) return;
        for (const listener of [...registered]) {
            try {
                listener(event.value);
            } catch (error) {
                console.error(
                    `A listener for the Event '${event.name}' threw.`, error
                );
            }
        }
    };

    const surface = { on, off, emit };

    for (const [name, value] of Object.entries(surface)) {
        Object.defineProperty(window.dry, name, {
            value: value,
            writable: false,
            configurable: false,
            enumerable: true,
        });
    }

    Object.defineProperty(window.dry, 'deliverEvent', {
        value: deliverEvent,
        writable: false,
        configurable: false,
        enumerable: false,
    });
})();
