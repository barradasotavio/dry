Object.assign(window, {
    messageMouseDown: (x, y) => window.ipc.postMessage(`window_control:mouse_down:${x},${y}`),
    drag: () => window.ipc.postMessage('window_control:drag'),
    minimize: () => window.ipc.postMessage('window_control:minimize'),
    toggleMaximize: () => window.ipc.postMessage('window_control:toggle_maximize'),
    close: () => window.ipc.postMessage('window_control:close'),
});