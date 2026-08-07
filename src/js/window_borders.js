// The eight resize edges of an undecorated window, drawn as thin fixed
// elements over the page border. This script is injected at document start,
// when `document.body` is still null, so the edges are only built once the
// document has a body to hold them.

(() => {
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

    const createEdge = (edge) => {
        const div = document.createElement('div');
        div.className = `resize-edge resize-${edge.position}`;
        // The edges sit outside any drag region, so a grab on the top edge
        // resizes instead of moving the window.
        div.setAttribute('data-no-drag-region', '');
        Object.assign(div.style, {
            position: 'fixed',
            zIndex: '9999',
            cursor: edge.cursor,
            ...edge.styles
        });
        div.addEventListener('mousedown', e => {
            e.preventDefault();
            window.dry.resize(edge.direction);
        });
        return div;
    };

    const install = () => {
        const edgeDivs = edgeSettings.map(edge => {
            const div = createEdge(edge);
            document.body.appendChild(div);
            return div;
        });

        window.addEventListener('resize', () => {
            const maximized = (
                window.innerWidth === window.screen.availWidth &&
                window.innerHeight === window.screen.availHeight
            );
            edgeDivs.forEach(div => {
                if (maximized) {
                    div.remove();
                } else if (div.parentNode !== document.body) {
                    document.body.appendChild(div);
                }
            });
        });
    };

    if (document.body) {
        install();
    } else {
        document.addEventListener('DOMContentLoaded', install, { once: true });
    }
})();
