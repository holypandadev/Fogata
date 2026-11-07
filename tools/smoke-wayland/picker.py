"""The person in front of the desktop portal's dialogs, in the smoke session
(tools/smoke-wayland/session.sh): it watches the accessibility bus for the dialogs KDE's
portal shows when an application asks to share the screen or to use the camera, and
answers them as a person would, with the pointer's own actions on the dialog's controls
(no shortcut around it).

    python3 picker.py <folder>

<folder>/answer says what to do with the next dialog: "share" (the default when the file is
missing) picks the first screen and presses Share, or grants the camera; "cancel" presses
Cancel, or denies the camera. Each dialog seen and what was done with it goes as a line to
<folder>/dialogs ("shown <n>", then "shared <n>", "cancelled <n>", "granted <n>" or
"denied <n>"), so a test can tell that the dialog came and was answered.
"""

import os
import sys
import time
import warnings

import gi

gi.require_version("Atspi", "2.0")
from gi.repository import Atspi  # noqa: E402

warnings.simplefilter("ignore", DeprecationWarning)
folder = sys.argv[1]
answered = set()
shown = 0


def log(line):
    print(f"{time.strftime('%H:%M:%S')} {line}", flush=True)


def note(line):
    with open(os.path.join(folder, "dialogs"), "a") as f:
        f.write(line + "\n")
    log(line)


def answer():
    try:
        with open(os.path.join(folder, "answer")) as f:
            return f.read().strip() or "share"
    except FileNotFoundError:
        return "share"


def walk(node, depth=0):
    """Every accessible under `node`, with its depth."""
    if node is None or depth > 40:
        return
    yield node, depth
    try:
        count = node.get_child_count()
    except Exception:
        return
    for i in range(count):
        try:
            child = node.get_child_at_index(i)
        except Exception:
            continue
        yield from walk(child, depth + 1)


def describe(node):
    try:
        states = node.get_state_set()
        flags = [
            s.value_nick
            for s in (Atspi.StateType.SHOWING, Atspi.StateType.ENABLED, Atspi.StateType.SELECTED)
            if states.contains(s)
        ]
        return f"{node.get_role_name()} '{node.get_name()}' [{','.join(flags)}]"
    except Exception as e:
        return f"? ({e})"


def act(node, names=("press", "click", "activate", "toggle", "select", "SetFocus")):
    """Does the first of the node's actions that a click does."""
    try:
        n = node.get_n_actions()
    except Exception:
        n = 0
    actions = [Atspi.Action.get_action_name(node, i) for i in range(n)]
    for wanted in names:
        for i, name in enumerate(actions):
            if name.lower() == wanted.lower():
                return node.do_action(i)
    if n:
        return node.do_action(0)
    return False


def enabled(node):
    try:
        return node.get_state_set().contains(Atspi.StateType.ENABLED)
    except Exception:
        return False


def portal_dialogs():
    desktop = Atspi.get_desktop(0)
    for i in range(desktop.get_child_count()):
        app = desktop.get_child_at_index(i)
        if app is None:
            continue
        # What a dialog shows changes as it opens and closes: never from the cache.
        try:
            app.clear_cache()
        except Exception:
            pass
        try:
            name = app.get_name() or ""
        except Exception:
            continue
        if "portal" not in name.lower():
            continue
        for j in range(app.get_child_count()):
            window = app.get_child_at_index(j)
            if window is None:
                continue
            try:
                showing = window.get_state_set().contains(Atspi.StateType.SHOWING)
            except Exception:
                continue
            if showing:
                yield app, window


def buttons(window, label):
    return [
        node
        for node, _ in walk(window)
        if node.get_role() in (Atspi.Role.PUSH_BUTTON, Atspi.Role.BUTTON)
        and (node.get_name() or "").replace("&", "").strip().lower() == label
    ]


def handle(app, window):
    global shown
    shown += 1
    n = shown
    note(f"shown {n}")
    for node, depth in walk(window):
        log("  " * depth + describe(node))
    what = answer()
    # The portal's question for a device (the camera): grant or deny, nothing to pick.
    # KDE's shows "Allow <app> to Use the Camera?" with OK and Cancel; others name the
    # buttons Grant Access and Deny Access.
    grant = buttons(window, "grant access") + buttons(window, "allow")
    deny = buttons(window, "deny access") + buttons(window, "deny")
    if "camera" in (window.get_name() or "").lower():
        grant += buttons(window, "ok")
        deny += buttons(window, "cancel")
    if grant or deny:
        if what == "cancel":
            if deny:
                act(deny[0])
            else:
                close_window(window.get_name())
            note(f"denied {n}")
        elif grant:
            act(grant[0])
            note(f"granted {n}")
        else:
            note(f"no grant button {n}")
        return
    if what == "cancel":
        for button in buttons(window, "cancel"):
            act(button)
            note(f"cancelled {n}")
            return
        # KDE's dialog has no Cancel button the accessibility bus shows: it is closed as a
        # person closes it, with its window's close button (KWin closes it as that does).
        close_window(window.get_name())
        note(f"cancelled {n}")
        return
    # The first screen in the dialog's list (a button showing the output's name; the
    # other buttons make a virtual screen or a region), then Share.
    special = {"share virtual screen", "share region", "remove", "clear search", "cancel",
               "share", "ok"}
    screens = [
        node
        for node, _ in walk(window)
        if node.get_role() in (Atspi.Role.PUSH_BUTTON, Atspi.Role.BUTTON)
        and showing(node)
        and (node.get_name() or "").strip()
        and (node.get_name() or "").replace("&", "").strip().lower() not in special
    ]
    if not screens:
        note(f"no screen to pick {n}")
        return
    log(f"picking {describe(screens[0])}")
    act(screens[0])
    for _ in range(40):
        time.sleep(0.25)
        if not any(w.path == window.path for _, w in portal_dialogs()):
            note(f"shared {n}")
            return
        share = [b for b in buttons(window, "share") + buttons(window, "ok") if enabled(b)]
        if share:
            for node, depth in walk(window):
                log("  " * depth + describe(node))
            act(share[0])
            note(f"shared {n}")
            return
    for node, depth in walk(window):
        log("  " * depth + describe(node))
    note(f"no share button {n}")


def close_window(caption):
    """Closes the windows titled `caption`, as their title bar's close button does."""
    import dbus

    script = os.path.join(folder, f"close-{time.monotonic_ns()}.js")
    with open(script, "w") as f:
        f.write(
            "for (const w of workspace.windowList()) {\n"
            f"  if (w.caption.startsWith({caption.strip()!r})) w.closeWindow();\n"
            "}\n"
        )
    kwin = dbus.SessionBus().get_object("org.kde.KWin", "/Scripting")
    number = kwin.loadScript(script, dbus_interface="org.kde.kwin.Scripting")
    dbus.SessionBus().get_object("org.kde.KWin", f"/Scripting/Script{number}").run(
        dbus_interface="org.kde.kwin.Script"
    )
    time.sleep(0.5)
    kwin.unloadScript(script, dbus_interface="org.kde.kwin.Scripting")
    os.remove(script)


def showing(node):
    try:
        return node.get_state_set().contains(Atspi.StateType.SHOWING)
    except Exception:
        return False


def main():
    Atspi.init()
    log("watching")
    while True:
        try:
            for app, window in portal_dialogs():
                key = (app.get_process_id(), window.path)
                if key in answered:
                    continue
                answered.add(key)
                handle(app, window)
            # A dialog that went can come again with the same name.
            current = {
                (a.get_process_id(), w.path) for a, w in portal_dialogs()
            }
            answered.intersection_update(current)
        except Exception as e:
            log(f"error: {e}")
        time.sleep(0.3)


if __name__ == "__main__":
    main()
