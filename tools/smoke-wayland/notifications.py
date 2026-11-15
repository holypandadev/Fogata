"""The desktop's notification server, in the smoke session (tools/smoke-wayland/session.sh).
KDE's portal tells the person through it, at every share, that the screen is being shared.
With none, the session bus tried to start one for 25 s at every share, and two shares in
thirty timed out in the application then (none since). This one shows nothing and answers
as a server that shows nothing would.

    python3 notifications.py
"""
import dbus
import dbus.service
from dbus.mainloop.glib import DBusGMainLoop
from gi.repository import GLib

NAME = "org.freedesktop.Notifications"


class Server(dbus.service.Object):
    def __init__(self, bus):
        super().__init__(bus, "/org/freedesktop/Notifications")
        self.last = 0

    @dbus.service.method(NAME, in_signature="susssasa{sv}i", out_signature="u")
    def Notify(self, app, replaces, icon, summary, body, actions, hints, timeout):
        if replaces:
            return replaces
        self.last += 1
        return self.last

    @dbus.service.method(NAME, in_signature="u", out_signature="")
    def CloseNotification(self, number):
        # 3: closed by a call to CloseNotification.
        self.NotificationClosed(number, 3)

    @dbus.service.method(NAME, in_signature="", out_signature="as")
    def GetCapabilities(self):
        return ["body"]

    @dbus.service.method(NAME, in_signature="", out_signature="ssss")
    def GetServerInformation(self):
        return ("smoke", "Fogata", "1", "1.2")

    @dbus.service.signal(NAME, signature="uu")
    def NotificationClosed(self, number, reason):
        pass


DBusGMainLoop(set_as_default=True)
bus = dbus.SessionBus()
name = dbus.service.BusName(NAME, bus)
server = Server(bus)
GLib.MainLoop().run()
