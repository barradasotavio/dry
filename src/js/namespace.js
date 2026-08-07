// Creates the single global the library owns. Everything Dry injects hangs off
// this object, so nothing the library adds can collide with a standard browser
// API or with the frontend's own globals. Locked down so page scripts cannot
// replace it.

(() => {
    if (window.dry) return;

    Object.defineProperty(window, 'dry', {
        value: {},
        writable: false,
        configurable: false,
        enumerable: true,
    });
})();
