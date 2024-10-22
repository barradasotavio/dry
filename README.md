# Dry: a tiny webview library for Python

Dry is an attempt to provide a minimalistic webview library for Python, built on top of [Wry](https://github.com/tauri-apps/wry). It is designed to be as simple as possible, with a focus on ease of use and minimal dependencies.

## Installation

Make sure you have Rust, Python and their respective package managers. My go-to choice for managing Python versions, environments and dependencies is [uv](https://github.com/astral-sh/uv).

```bash
git clone
cd dry
uv sync
uv run maturin develop --uv
uv run .\prototypes\main.py
```