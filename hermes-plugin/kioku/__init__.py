"""Kioku for Hermes: passes Hermes's hook payloads to `kioku hook hermes <event>` and hands its
answers back. Installed and enabled by Kioku's install step (it links this folder into
~/.hermes/plugins/kioku and adds kioku to plugins.enabled). It never breaks a turn: on any
problem it does nothing."""

import json
import logging
import os
import subprocess
from pathlib import Path

log = logging.getLogger(__name__)
_compacted = set()


def _kioku_bin():
    if os.environ.get("KIOKU_BIN"):
        return os.environ["KIOKU_BIN"]
    cfg = Path(os.environ.get("XDG_CONFIG_HOME") or Path.home() / ".config")
    for line in (cfg / "kioku" / "installed").read_text().splitlines():
        if line.startswith("vault="):
            return str(Path(line[6:]) / ".kioku" / "bin" / "kioku")
    raise FileNotFoundError("no vault= line in the Kioku install record")


def _hook(event, payload, timeout=25):
    try:
        r = subprocess.run([_kioku_bin(), "hook", "hermes", event], input=json.dumps(payload),
                           capture_output=True, text=True, timeout=timeout)
        out = r.stdout.strip()
        return json.loads(out) if out else {}
    except Exception as e:  # a hook must never break the turn
        log.debug("kioku hook %s: %s", event, e)
        return {}


def _pre_llm_call(session_id="", is_first_turn=False, platform="", **kwargs):
    compacted = session_id in _compacted
    _compacted.discard(session_id)
    event = "session-start" if is_first_turn or compacted else "prompt"
    out = _hook(event, {"session_id": session_id, "platform": platform, "cwd": os.getcwd(),
                        "source": "compact" if compacted else ""})
    return {"context": out["context"]} if out.get("context") else None


def _transform_tool_result(tool_name="", args=None, result="", session_id="", **kwargs):
    if not isinstance(result, str):
        return None
    out = _hook("tool", {"session_id": session_id, "tool_name": tool_name, "args": args or {},
                         "result": result[:20000], "cwd": os.getcwd()}, timeout=15)
    if out.get("append"):
        return f"{result}\n\n{out['append']}"
    return None


def _pre_verify(session_id="", attempt=0, **kwargs):
    if attempt:
        return None
    out = _hook("stop", {"session_id": session_id}, timeout=15)
    return {"action": "continue", "message": out["message"]} if out.get("message") else None


def _post_compaction(session_id="", **kwargs):
    _compacted.add(session_id)


def register(ctx):
    ctx.register_hook("pre_llm_call", _pre_llm_call)
    ctx.register_hook("transform_tool_result", _transform_tool_result)
    ctx.register_hook("pre_verify", _pre_verify)
    ctx.register_hook("post_compaction", _post_compaction)
