// Turns a mousedown inside a drag region into a window drag, and a double
// click into a maximize toggle. A drag region is any element carrying
// `data-drag-region`; the whole subtree under it drags, so a title or an icon
// inside a custom titlebar works like the bare titlebar does. An element
// carrying `data-no-drag-region` opts itself and its subtree back out, which
// is what the window control buttons sitting inside the titlebar need.

(() => {
    const DRAG_SELECTOR = '[data-drag-region], [data-no-drag-region]';

    class DragChecker {
        constructor(threshold = 1) {
            this.startX = 0;
            this.startY = 0;
            this.isDragging = false;
            this.threshold = threshold;
        }

        handleMouseUp = () => {
            this.stop();
        }

        checkDragThreshold = (e) => {
            if (!this.isDragging && (
                Math.abs(e.clientX - this.startX) > this.threshold ||
                Math.abs(e.clientY - this.startY) > this.threshold
            )) {
                this.isDragging = true;
                this.stop();
                window.dry.drag();
            }
        }

        start(x, y) {
            this.startX = x;
            this.startY = y;
            this.isDragging = false;
            document.addEventListener('mousemove', this.checkDragThreshold);
            document.addEventListener('mouseup', this.handleMouseUp);
        }

        stop() {
            document.removeEventListener('mousemove', this.checkDragThreshold);
            document.removeEventListener('mouseup', this.handleMouseUp);
            this.isDragging = false;
        }
    }

    const dragChecker = new DragChecker();

    // The nearest ancestor that opts in or out wins, so an opt-out nested
    // inside a drag region is honoured, and a drag region nested inside an
    // opt-out drags again.
    const isDragRegion = (target) => {
        if (!(target instanceof Element)) return false;
        const marked = target.closest(DRAG_SELECTOR);
        return marked !== null && marked.hasAttribute('data-drag-region');
    };

    document.addEventListener('mousedown', (e) => {
        if (!isDragRegion(e.target)) return;

        const isMainMouseButton = e.button === 0;
        if (!isMainMouseButton) { return; }

        const isDoubleClick = e.detail === 2;
        if (isDoubleClick) {
            window.dry.toggleMaximize();
        } else {
            dragChecker.start(e.clientX, e.clientY);
        }
    });
})();
