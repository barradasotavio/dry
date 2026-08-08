# Contents

<!-- mdBook reads this file as the book's table of contents. The name is
mdBook's, not a choice: `mdbook build` looks for `SUMMARY.md` in the source
directory and builds the sidebar from the list below. -->

[Dry](./introduction.md)

# Getting started

- [Installing](./installing.md)
- [Your first Webview](./first-webview.md)

# Content

- [The three Content modes](./content.md)
- [Serving a Root](./root.md)
- [Loading a URL from a local server](./local-servers.md)

# The Bridge

- [Calls: the frontend asks Python](./calls.md)
- [Events: both directions](./events.md)
- [The Bridge contract](./contract.md)
- [Converting your own types](./default-hook.md)

# The window

- [Window options](./window-options.md)
- [Custom titlebars](./titlebar.md)
- [Window Events](./window-events.md)
- [Runtime window control](./runtime-control.md)
- [Closing the window](./close-hook.md)

# How Dry runs

- [The Portal](./portal.md)
- [Where your data lives](./app-data.md)
- [Errors and logging](./errors.md)

# Reference

- [The Webview](./reference-webview.md)
- [The window.dry namespace](./reference-javascript.md)

# Releases

- [Migrating to 0.4.0](./migration-0.4.md)
- [Changelog](./changelog.md)

# Decisions

- [ADR-0001: Dry owns the process and the asyncio loop](./decisions/0001.md)
- [ADR-0002: The Bridge contract is the JSON data model](./decisions/0002.md)
- [ADR-0003: abi3 wheels with a Python 3.14 floor](./decisions/0003.md)
- [ADR-0004: macOS resizes an undecorated window from the frontend](./decisions/0004.md)
