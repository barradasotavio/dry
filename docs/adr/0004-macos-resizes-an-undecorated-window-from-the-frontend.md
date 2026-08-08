# macOS resizes an undecorated window from the frontend

An undecorated window draws eight resize edges, and each one hands its grab to `tao`'s `drag_resize_window`, which lets the platform take the drag over. macOS has no such platform path: `tao` 0.36 implements `drag_resize_window` in its macOS backend as an unconditional `Err(NotSupported)`. The message reached Rust, Rust logged a failure, and the window did not move — a frontend was being handed eight resize handles that could not resize anything, which reads as a bug in the frontend's own code rather than in ours.

Three ways out were open: implement the drag natively through `NSWindow`, draw no edges on macOS at all, or document that they do nothing. Nothing is the worst of the three, so it was never really in the running. Drawing no edges is honest, but it costs every macOS user of an undecorated window the ability to resize it, which is most of the reason to reach for one. The native implementation needs `objc2` in `Cargo.toml`, a hand-written `NSEvent` tracking loop, and unsafe message sends on a platform we ship — a large surface for the size of the problem.

So on macOS `dry.resize(direction)` runs the drag itself: it tracks the pointer, reports it to Rust on every move, and Rust moves the grabbed edges to it with `set_outer_position` and `set_inner_size`. Windows is untouched and still hands the grab to the platform, which is both cheaper and better behaved than anything we would write. The split lives in `src/js/window_functions.js` and `src/window/resize.rs`.

## Consequences

The pointer travels in client coordinates. WebKit reports `window.screenX` as 0 for an undecorated window whatever the window's real position, so screen coordinates are not available to be trusted; the window's own frame of reference is.

Every report is absolute rather than incremental — each one asks for the geometry the pointer implies, given the frame the window has now — so a report that is coalesced, dropped or late costs a frame and not a permanent drift. What a drag holds still is pinned once, when the edge is grabbed: macOS refuses to lift a window's top above the menu bar, and without a pinned bottom edge that refusal would walk the window down the screen for as long as the user kept pushing up.

A resize the platform drives is clamped to the window's minimum size by the platform; a `set_inner_size` is not. The minimum is therefore recorded when the window is built and enforced in `resize.rs`.

The macOS path is a frontend loop rather than a platform one, so it repaints per report rather than per frame, and a resize under heavy JavaScript load will trail the cursor in a way the Windows path does not.
