document.addEventListener('mousedown', (e) => {
    const isMainMouseButton = e.button === 0;
    if (!isMainMouseButton) { return; }

    const isDragRegion = e.target.hasAttribute('data-drag-region');
    if (!isDragRegion) { window.messageMouseDown(e.clientX, e.clientY); return; }

    const isDoubleClick = e.detail === 2;
    if (isDoubleClick) { window.toggleMaximize(); }
    else { window.drag(); }
})

document.addEventListener('touchstart', (e) => {
    const isDragRegion = e.target.hasAttribute('data-drag-region');
    if (isDragRegion) window.drag();
})