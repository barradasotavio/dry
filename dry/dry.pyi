from typing import Any, Callable, TypedDict

class Settings(TypedDict):
    title: str
    min_size: tuple[int, int]
    size: tuple[int, int]
    decorations: bool | None
    icon_path: str | None
    html: str | None
    url: str | None
    api: dict[str, Callable[..., Any]] | None
    dev_tools: bool | None
    user_data_folder: str

def run(
    settings: Settings,
) -> None:
    ...

def send_event(message: str) -> None:
    ...
