from typing import Any, Callable

def run(
    title: str,
    min_width: int,
    min_height: int,
    width: int,
    height: int,
    html: str,
    api: dict[str, Callable[..., Any]],
    initialization_script: str,
) -> None: ...
