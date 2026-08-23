#!/usr/bin/env python3
"""Minimal tui_gateway stand-in for hermes-rust tests. Speaks NDJSON JSON-RPC on stdio."""
from __future__ import annotations

import json
import sys
import time


def emit(obj: dict) -> None:
    sys.stdout.write(json.dumps(obj, ensure_ascii=False) + "\n")
    sys.stdout.flush()


def main() -> None:
    emit(
        {
            "jsonrpc": "2.0",
            "method": "event",
            "params": {
                "type": "gateway.ready",
                "payload": {
                    "skin": {"name": "fake-dark", "mode": "dark"},
                    "change_events": True,
                },
            },
        }
    )
    for raw in sys.stdin:
        line = raw.strip()
        if not line:
            continue
        try:
            req = json.loads(line)
        except json.JSONDecodeError:
            emit(
                {
                    "jsonrpc": "2.0",
                    "id": None,
                    "error": {"code": -32700, "message": "parse error"},
                }
            )
            continue
        method = req.get("method")
        rid = req.get("id")
        params = req.get("params") or {}
        if method == "session.create":
            emit(
                {
                    "jsonrpc": "2.0",
                    "id": rid,
                    "result": {
                        "session_id": "sess-fake",
                        "stored_session_id": "store-fake",
                        "message_count": 0,
                        "messages": [],
                        "info": {"lazy": True, "cwd": params.get("cwd", ".")},
                    },
                }
            )
            time.sleep(0.02)
            emit(
                {
                    "jsonrpc": "2.0",
                    "method": "event",
                    "params": {
                        "type": "session.info",
                        "session_id": "sess-fake",
                        "payload": {
                            "model": "fake-model",
                            "cwd": params.get("cwd", "."),
                            "lazy": False,
                        },
                    },
                }
            )
        elif method == "prompt.submit":
            result = {"ok": True, "status": "streaming"}
            if params.get("confirm_truncate"):
                result["survivor_user_row_ids"] = (
                    [] if params.get("confirm_empty_truncate") else [1]
                )
            emit({"jsonrpc": "2.0", "id": rid, "result": result})
            text = (params.get("text") or "hello") + " from fake gateway."
            emit(
                {
                    "jsonrpc": "2.0",
                    "method": "event",
                    "params": {
                        "type": "message.delta",
                        "session_id": params.get("session_id"),
                        "payload": {"text": text, "delta": text},
                    },
                }
            )
            emit(
                {
                    "jsonrpc": "2.0",
                    "method": "event",
                    "params": {
                        "type": "message.complete",
                        "session_id": params.get("session_id"),
                        "payload": {"text": text},
                    },
                }
            )
        elif method == "session.close":
            emit({"jsonrpc": "2.0", "id": rid, "result": {"ok": True}})
        elif method == "process.stop":
            emit({"jsonrpc": "2.0", "id": rid, "result": {"killed": True}})
            return
        elif method == "terminal.resize":
            emit({"jsonrpc": "2.0", "id": rid, "result": {"ok": True}})
        elif method == "session.interrupt":
            emit({"jsonrpc": "2.0", "id": rid, "result": {"ok": True}})
        elif method == "commands.catalog":
            emit(
                {
                    "jsonrpc": "2.0",
                    "id": rid,
                    "result": {
                        "pairs": [
                            ["help", "show help"],
                            ["resume", "resume a saved session"],
                            ["sessions", "browse sessions"],
                            ["clear", "clear the transcript"],
                        ]
                    },
                }
            )
        elif method == "session.list":
            emit(
                {
                    "jsonrpc": "2.0",
                    "id": rid,
                    "result": {
                        "sessions": [
                            {
                                "id": "store-fake",
                                "title": "Fake saved",
                                "preview": "hello",
                                "started_at": 0,
                                "message_count": 2,
                                "source": "tui",
                            }
                        ]
                    },
                }
            )
        elif method == "session.active_list":
            emit(
                {
                    "jsonrpc": "2.0",
                    "id": rid,
                    "result": {
                        "sessions": [
                            {
                                "id": "sess-fake",
                                "title": "current",
                                "status": "idle",
                                "current": True,
                            }
                        ]
                    },
                }
            )
        elif method == "session.resume":
            emit(
                {
                    "jsonrpc": "2.0",
                    "id": rid,
                    "result": {
                        "session_id": "sess-resumed",
                        "resumed": params.get("session_id"),
                        "messages": [
                            {"role": "user", "text": "old question", "row_id": 11},
                            {"role": "assistant", "text": "old answer", "row_id": 12},
                        ],
                        "info": {"model": "fake-model", "lazy": True},
                    },
                }
            )
        elif method == "session.history":
            emit(
                {
                    "jsonrpc": "2.0",
                    "id": rid,
                    "result": {
                        "count": 2,
                        "messages": [
                            {"role": "user", "text": "old question", "row_id": 11},
                            {"role": "assistant", "text": "old answer", "row_id": 12},
                        ],
                    },
                }
            )
        elif method == "session.activate":
            emit(
                {
                    "jsonrpc": "2.0",
                    "id": rid,
                    "result": {
                        "session_id": params.get("session_id"),
                        "messages": [],
                        "info": {"model": "fake-model"},
                    },
                }
            )
        elif method == "command.dispatch":
            emit(
                {
                    "jsonrpc": "2.0",
                    "id": rid,
                    "result": {
                        "type": "exec",
                        "output": f"fake ran {params.get('command')}",
                    },
                }
            )
        elif method in (
            "approval.respond",
            "clarify.respond",
            "sudo.respond",
            "secret.respond",
            "session.steer",
            "subagent.interrupt",
        ):
            emit({"jsonrpc": "2.0", "id": rid, "result": {"ok": True}})
        elif method == "session.branch":
            emit(
                {
                    "jsonrpc": "2.0",
                    "id": rid,
                    "result": {
                        "session_id": "sess-branch",
                        "title": "branch",
                        "messages": [],
                        "info": {"model": "fake-model"},
                    },
                }
            )
        elif method == "delegation.status":
            emit(
                {
                    "jsonrpc": "2.0",
                    "id": rid,
                    "result": {
                        "active": [
                            {
                                "subagent_id": "sa-0",
                                "goal": "fake child",
                                "status": "running",
                            }
                        ]
                    },
                }
            )
        elif method == "clipboard.paste":
            emit(
                {
                    "jsonrpc": "2.0",
                    "id": rid,
                    "result": {"attached": False, "message": "no image (fake)"},
                }
            )
        elif method == "spawn_tree.list":
            emit(
                {
                    "jsonrpc": "2.0",
                    "id": rid,
                    "result": {
                        "entries": [
                            {
                                "path": "fake-tree.json",
                                "label": "fake tree",
                                "count": 1,
                            }
                        ]
                    },
                }
            )
        elif method == "spawn_tree.load":
            emit(
                {
                    "jsonrpc": "2.0",
                    "id": rid,
                    "result": {"label": params.get("path"), "session_id": "sess-fake"},
                }
            )
        elif method == "image.attach":
            emit(
                {
                    "jsonrpc": "2.0",
                    "id": rid,
                    "result": {
                        "attached": True,
                        "text": f"[User attached image: {params.get('path')}]",
                    },
                }
            )
        else:
            emit({"jsonrpc": "2.0", "id": rid, "result": {}})
    # genuine stdin EOF


if __name__ == "__main__":
    main()
