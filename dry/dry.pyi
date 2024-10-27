from typing import Any, Callable

def run(
    title: str,
    min_size: tuple[int, int],
    size: tuple[int, int],
    html: str,
    startup_script: str,
    api: dict[str, Callable[..., Any]],
) -> None: ...
