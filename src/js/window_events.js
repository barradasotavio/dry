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
            window.drag();
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

document.querySelectorAll('[data-drag-region]').forEach(dragRegion => {
    dragRegion.addEventListener('mousedown', (e) => {
        const isMainMouseButton = e.button === 0;
        if (!isMainMouseButton) { return; }

        const isDoubleClick = e.detail === 2;
        if (isDoubleClick) {
            window.toggleMaximize();
        } else {
            dragChecker.start(e.clientX, e.clientY);
        }
    });
});