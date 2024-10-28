from typing import Any, Callable, Mapping

def run(
    title: str,
    min_size: tuple[int, int],
    size: tuple[int, int],
    icon_path: str | None = None,
    html: str | None = None,
    url: str | None = None,
    api: Mapping[str, Callable[..., Any]] | None = None,
    dev_tools: bool | None = None,
) -> None: ...
