// Window controls, exposed as dry.minimize(), dry.toggleMaximize(),
// dry.close(), dry.drag(), dry.resize(direction) and dry.state().

(() => {
    const send = (message) => window.ipc.postMessage(message);

    // dry.state() is the complement to the window Events: they say what
    // changed, this says what is. A page that has just loaded has observed
    // nothing and still has to draw its maximize button one way round.
    //
    // It returns a Promise because the answer comes from the thread that owns
    // the window. Every call waiting on the same trip is resolved with the
    // same reading — a reading taken now answers every question asked since
    // the last one, so a page polling it costs one trip either way. Rust
    // resolves them through dry.resolveState, non-enumerable and non-writable
    // like dry.resolveCall.
    const waiting = [];

    const state = () => new Promise((resolve) => {
        waiting.push(resolve);
        send('window_control:state');
    });

    const resolveState = (reading) => {
        for (const resolve of waiting.splice(0)) resolve(reading);
    };

    Object.defineProperty(window.dry, 'resolveState', {
        value: resolveState,
        writable: false,
        configurable: false,
        enumerable: false,
    });

    // macOS has no native drag-resize to hand a grab to — tao answers
    // NotSupported there — so dry.resize() runs the drag here instead, and
    // reports the pointer on every move to a Rust side that moves the grabbed
    // edges to it. Windows hands the grab to the platform, which takes over
    // with a modal loop of its own. See ADR-0004.
    const dragsHere = /Mac/i.test(navigator.platform || navigator.userAgent);

    // dry.resize() is called from a mousedown handler and is handed no event,
    // so the grab it belongs to is tracked here. A call with no button down is
    // not a grab, and starts nothing.
    let grab = null;
    if (dragsHere) {
        window.addEventListener('mousedown', e => {
            grab = { x: e.clientX, y: e.clientY };
        }, true);
        window.addEventListener('mouseup', () => {
            grab = null;
        }, true);
    }

    // Where the grab sits relative to the edges it drags, so the window does
    // not jump by the thickness of the handle on the first move. The pointer
    // travels in client coordinates, the one frame of reference WebKit reports
    // faithfully for an undecorated window: window.screenX is 0 there whatever
    // the window's real position.
    const grabOffset = (direction) => [
        direction.endsWith('west') ? grab.x
            : direction.endsWith('east') ? grab.x - window.innerWidth
                : 0,
        direction.startsWith('north') ? grab.y
            : direction.startsWith('south') ? grab.y - window.innerHeight
                : 0,
    ];

    const dragEdges = (direction) => {
        const offset = grabOffset(direction);
        const report = (phase, x, y) => send(
            `window_control:resize_drag:${phase}:${direction}` +
            `:${Math.round(x)}:${Math.round(y)}` +
            `:${Math.round(offset[0])}:${Math.round(offset[1])}`
        );
        const move = (event) => {
            event.preventDefault();
            report('move', event.clientX, event.clientY);
        };
        const stop = () => {
            window.removeEventListener('mousemove', move, true);
            window.removeEventListener('mouseup', stop, true);
        };
        report('grab', grab.x, grab.y);
        window.addEventListener('mousemove', move, true);
        window.addEventListener('mouseup', stop, true);
    };

    const resize = (direction) => {
        if (!dragsHere) {
            send(`window_control:resize:${direction}`);
        } else if (grab) {
            dragEdges(direction);
        }
    };

    const controls = {
        drag: () => send('window_control:drag'),
        minimize: () => send('window_control:minimize'),
        toggleMaximize: () => send('window_control:toggle_maximize'),
        close: () => send('window_control:close'),
        resize,
        state,
    };

    for (const [name, control] of Object.entries(controls)) {
        Object.defineProperty(window.dry, name, {
            value: control,
            writable: false,
            configurable: false,
            enumerable: true,
        });
    }
})();
