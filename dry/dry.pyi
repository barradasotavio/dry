from typing import Callable, Dict

def run(
    title: str,
    min_width: int,
    min_height: int,
    width: int,
    height: int,
    html: str,
    api: Dict[str, Callable[..., str]],
) -> None: ...
