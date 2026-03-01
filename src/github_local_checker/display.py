try:
    from .constants import _COLOR, _RESET
except ImportError:
    from constants import _COLOR, _RESET


def colored(text: str, status: str) -> str:
    return f"{_COLOR.get(status, '')}{text}{_RESET}"
