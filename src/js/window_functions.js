// Window controls, exposed as dry.minimize(), dry.toggleMaximize(),
// dry.close(), dry.drag() and dry.resize(direction).

(() => {
    const send = (message) => window.ipc.postMessage(message);

    const controls = {
        drag: () => send('window_control:drag'),
        minimize: () => send('window_control:minimize'),
        toggleMaximize: () => send('window_control:toggle_maximize'),
        close: () => send('window_control:close'),
        resize: (direction) => send(`window_control:resize:${direction}`),
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
