const edgeSettings = [
    { position: 'top', styles: { top: '0', left: '0', right: '0', height: '3px' }, cursor: 'n-resize', direction: 'north' },
    { position: 'right', styles: { top: '0', right: '0', bottom: '0', width: '3px' }, cursor: 'e-resize', direction: 'east' },
    { position: 'bottom', styles: { left: '0', right: '0', bottom: '0', height: '3px' }, cursor: 's-resize', direction: 'south' },
    { position: 'left', styles: { top: '0', left: '0', bottom: '0', width: '3px' }, cursor: 'w-resize', direction: 'west' },
    { position: 'top-left', styles: { top: '0', left: '0', width: '7px', height: '7px' }, cursor: 'nw-resize', direction: 'north-west' },
    { position: 'top-right', styles: { top: '0', right: '0', width: '7px', height: '7px' }, cursor: 'ne-resize', direction: 'north-east' },
    { position: 'bottom-left', styles: { left: '0', bottom: '0', width: '7px', height: '7px' }, cursor: 'sw-resize', direction: 'south-west' },
    { position: 'bottom-right', styles: { right: '0', bottom: '0', width: '7px', height: '7px' }, cursor: 'se-resize', direction: 'south-east' }
];

const edgeDivs = edgeSettings.map(edge => {
    const div = document.createElement('div');
    div.className = `resize-edge resize-${edge.position}`;
    Object.assign(div.style, {
        position: 'fixed',
        zIndex: '9999',
        cursor: edge.cursor,
        ...edge.styles
    });
    div.addEventListener('mousedown', e => {
        e.preventDefault();
        window.resize(edge.direction);
    });
    document.body.appendChild(div);
    return div;
});

window.addEventListener('resize', () => {
    const maximized = (
        window.innerWidth === window.screen.availWidth &&
        window.innerHeight === window.screen.availHeight
    );
    edgeDivs.forEach(div => {
        maximized ? document.body.removeChild(div) : document.body.appendChild(div);
    });
});
