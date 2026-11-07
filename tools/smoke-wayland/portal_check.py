"""Shares the screen once through the desktop portal (org.freedesktop.portal.ScreenCast) on
the session bus, the way an application does, and closes it again: the check that the smoke
session (tools/smoke-wayland/session.sh) offers screen sharing before the copies of the app
run. The portal's dialog is answered by tools/smoke-wayland/picker.py. Prints what it gets;
exits 0 when the portal gave a PipeWire stream and a remote to read it from, 1 otherwise."""

import sys

import dbus
import dbus.mainloop.glib
from gi.repository import GLib

dbus.mainloop.glib.DBusGMainLoop(set_as_default=True)
bus = dbus.SessionBus()
sender = bus.get_unique_name()[1:].replace(".", "_")
portal = bus.get_object("org.freedesktop.portal.Desktop", "/org/freedesktop/portal/desktop")
cast = dbus.Interface(portal, "org.freedesktop.portal.ScreenCast")
loop = GLib.MainLoop()
result = {"ok": False}
count = [0]


def request(method, *args, options):
    """Calls `method` with a handle token and waits for the request's Response."""
    count[0] += 1
    token = f"smoke{count[0]}"
    path = f"/org/freedesktop/portal/desktop/request/{sender}/{token}"
    answer = {}

    def on_response(code, results):
        answer["code"] = int(code)
        answer["results"] = results
        loop.quit()

    match = bus.add_signal_receiver(
        on_response,
        "Response",
        "org.freedesktop.portal.Request",
        "org.freedesktop.portal.Desktop",
        path,
    )
    options["handle_token"] = token
    method(*args, options)
    timer = GLib.timeout_add_seconds(90, loop.quit)
    loop.run()
    GLib.source_remove(timer)
    match.remove()
    if "code" not in answer:
        raise RuntimeError(f"no answer to {method._method_name}")
    return answer["code"], answer["results"]


def main():
    version = portal.Get(
        "org.freedesktop.portal.ScreenCast",
        "version",
        dbus_interface="org.freedesktop.DBus.Properties",
    )
    types = portal.Get(
        "org.freedesktop.portal.ScreenCast",
        "AvailableSourceTypes",
        dbus_interface="org.freedesktop.DBus.Properties",
    )
    print(f"ScreenCast version {version}, source types {int(types)}")
    code, results = request(
        cast.CreateSession, options={"session_handle_token": "smokecheck"}
    )
    if code != 0:
        raise RuntimeError(f"CreateSession answered {code}")
    session = results["session_handle"]
    code, _ = request(
        cast.SelectSources,
        session,
        options={"types": dbus.UInt32(1), "multiple": False},
    )
    if code != 0:
        raise RuntimeError(f"SelectSources answered {code}")
    code, results = request(cast.Start, session, "", options={})
    if code != 0:
        raise RuntimeError(f"Start answered {code} (the dialog was refused or failed)")
    streams = results.get("streams", [])
    print(f"streams: {[(int(node), dict(props)) for node, props in streams]}")
    if not streams:
        raise RuntimeError("no stream")
    fd = cast.OpenPipeWireRemote(session, {})
    print(f"PipeWire remote: fd {fd.take()}")
    bus.get_object("org.freedesktop.portal.Desktop", session).Close(
        dbus_interface="org.freedesktop.portal.Session"
    )
    result["ok"] = True


if __name__ == "__main__":
    try:
        main()
    except Exception as e:  # The reason goes to the log the session names.
        print(f"portal check failed: {e}")
    sys.exit(0 if result["ok"] else 1)
