Object.assign(window, {
    resize: (direction) => window.ipc.postMessage(`window_control:resize:${direction}`),
    drag: () => window.ipc.postMessage('window_control:drag'),
    minimize: () => window.ipc.postMessage('window_control:minimize'),
    toggleMaximize: () => window.ipc.postMessage('window_control:toggle_maximize'),
    close: () => window.ipc.postMessage('window_control:close'),
});