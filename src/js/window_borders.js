document.addEventListener('DOMContentLoaded', () => {
    const style = document.createElement('style');
    style.textContent = '*[data-drag-region] { app-region: drag; }';
    document.head.appendChild(style);

    const edgeThickness = '5px';
    const body = document.body;

    const edges = [
        { position: 'top', cursor: 'n-resize' },
        { position: 'right', cursor: 'e-resize' },
        { position: 'bottom', cursor: 's-resize' },
        { position: 'left', cursor: 'w-resize' },
        { position: 'top-left', cursor: 'nw-resize' },
        { position: 'top-right', cursor: 'ne-resize' },
        { position: 'bottom-left', cursor: 'sw-resize' },
        { position: 'bottom-right', cursor: 'se-resize' }
    ];

    const edgeStyles = {
        top: { top: '0', left: '0', right: '0', height: edgeThickness },
        right: { top: '0', right: '0', bottom: '0', width: edgeThickness },
        bottom: { left: '0', right: '0', bottom: '0', height: edgeThickness },
        left: { top: '0', left: '0', bottom: '0', width: edgeThickness },
        'top-left': { top: '0', left: '0', width: edgeThickness, height: edgeThickness },
        'top-right': { top: '0', right: '0', width: edgeThickness, height: edgeThickness },
        'bottom-left': { bottom: '0', left: '0', width: edgeThickness, height: edgeThickness },
        'bottom-right': { bottom: '0', right: '0', width: edgeThickness, height: edgeThickness }
    };

    const edgeDivs = [];

    edges.forEach(edge => {
        const div = document.createElement('div');
        div.className = `resize-edge resize-${edge.position}`;
        div.style.position = 'fixed';
        div.style.zIndex = '9999';
        div.style.cursor = edge.cursor;
        div.style.backgroundColor = 'transparent';
        const styles = edgeStyles[edge.position] || {};
        Object.assign(div.style, styles);
        body.appendChild(div);
        edgeDivs.push(div);
    });
});


