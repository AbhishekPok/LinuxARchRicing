"""Hyprland fullscreen watcher; prints 1 when a real fullscreen window is
visible, 0 otherwise, one per line.

Replaces the Mango original's direct socket watch. Written in Python for the
same reason clients.py is: the shell crate has no JSON dependency, and
Hyprland's client list has to be decoded, not just pattern-matched.

Hyprland's `fullscreen` is an enum: 0 none, 1 maximized, 2 fullscreen. Only 2
hides the island -- a maximized window should not.
"""
import json
import os
import socket
import time

WAKE = (
    "fullscreen", "openwindow", "closewindow", "workspace", "focusedmon",
    "activewindow", "movewindow", "monitoradded", "monitorremoved",
)


def socket_dir():
    runtime = os.environ.get("XDG_RUNTIME_DIR", "/run/user/%d" % os.getuid())
    signature = os.environ.get("HYPRLAND_INSTANCE_SIGNATURE")
    return os.path.join(runtime, "hypr", signature) if signature else None


def request(payload):
    path = socket_dir()
    if not path:
        return None
    try:
        with socket.socket(socket.AF_UNIX) as connection:
            connection.settimeout(2)
            connection.connect(os.path.join(path, ".socket.sock"))
            connection.sendall(payload.encode())
            chunks = []
            while True:
                chunk = connection.recv(65536)
                if not chunk:
                    break
                chunks.append(chunk)
        return b"".join(chunks).decode(errors="replace")
    except OSError:
        return None


def any_fullscreen():
    """True when a fullscreen window sits on its monitor's active workspace."""
    try:
        clients = json.loads(request("-j/clients") or "[]")
        monitors = json.loads(request("-j/monitors") or "[]")
    except (ValueError, TypeError):
        return False
    if not isinstance(clients, list) or not isinstance(monitors, list):
        return False
    visible = set()
    for m in monitors:
        if isinstance(m, dict):
            active = (m.get("activeWorkspace") or {}).get("id")
            if isinstance(active, int):
                visible.add(active)
    for c in clients:
        if not isinstance(c, dict) or c.get("hidden") or not c.get("mapped", True):
            continue
        if c.get("fullscreen") != 2:
            continue
        if ((c.get("workspace") or {}).get("id")) in visible:
            return True
    return False


def exit_with_parent():
    """amane-shell reads our stdout; once it is gone nobody does, and a quiet
    Hyprland may never make us print (and fail) again, so watch for it."""
    parent = os.getppid()
    while os.getppid() == parent:
        time.sleep(5)
    os._exit(0)


if __name__ == "__main__":
    import threading

    threading.Thread(target=exit_with_parent, daemon=True).start()
    last = None
    while True:
        path = socket_dir()
        if path:
            try:
                with socket.socket(socket.AF_UNIX) as connection:
                    connection.connect(os.path.join(path, ".socket2.sock"))
                    state = any_fullscreen()
                    print(int(state), flush=True)
                    last = state
                    for line in connection.makefile(errors="replace"):
                        if line.split(">>", 1)[0].strip() not in WAKE:
                            continue
                        state = any_fullscreen()
                        if state != last:
                            last = state
                            print(int(state), flush=True)
            except BrokenPipeError:
                # the shell went away; BrokenPipeError is an OSError, so
                # without this the loop below would retry forever as an orphan
                os._exit(0)
            except OSError:
                pass
        if last is not False:
            print(0, flush=True)
            last = False
        time.sleep(1)
