"""Content-free Hermes session identity; stdlib-only, bounded and silent."""
from functools import partial
import json
import os
from pathlib import Path
import re
import subprocess

_ID = re.compile(r"[A-Za-z0-9._:-]{1,128}\Z")
_PLATFORMS = {"cli", "tui", "desktop", "acp"}
_SCRIPT = Path(__file__).resolve().parent.parent / "scripts" / "forward-hook.mjs"


def _report(event, **kwargs):
    platform = kwargs.get("platform")
    session = kwargs.get("session_id")
    if not isinstance(platform, str) or platform not in _PLATFORMS:
        return
    if event == "pre_llm_call" and platform != "cli":
        return
    if not isinstance(session, str) or _ID.fullmatch(session) is None:
        return
    payload = json.dumps({"event": event, "platform": platform, "session_id": session}).encode("utf-8")
    try:
        subprocess.run(
            ["node", str(_SCRIPT), "hermes"],
            input=payload,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            timeout=2,
            check=False,
            creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0,
        )
    except (OSError, subprocess.SubprocessError):
        pass


def register(ctx):
    for event in ("on_session_start", "on_session_reset", "pre_llm_call"):
        ctx.register_hook(event, partial(_report, event))
