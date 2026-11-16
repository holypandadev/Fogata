#include "include/desktop_drop/desktop_drop_plugin.h"

#include <flutter_linux/flutter_linux.h>
#include <gtk/gtk.h>
#include <stdlib.h>
#include <sys/utsname.h>

#define DESKTOP_DROP_PLUGIN(obj) \
  (G_TYPE_CHECK_INSTANCE_CAST((obj), desktop_drop_plugin_get_type(), \
                              DesktopDropPlugin))

struct _DesktopDropPlugin {
  GObject parent_instance;
};

static gboolean isKDE = FALSE;
static gboolean ignoreNext = FALSE;

G_DEFINE_TYPE(DesktopDropPlugin, desktop_drop_plugin, g_object_get_type())

// Fogata: the targets through which a source hands its files to a sandboxed application
// (xdg-desktop-portal's FileTransfer; GTK 3 offers the second, older name too), taken only
// inside the Flatpak (see the targets below).
static const gchar kPortalTarget[] = "application/vnd.portal.filetransfer";
static const gchar kPortalFilesTarget[] = "application/vnd.portal.files";

// Fogata: whether this runs inside a Flatpak (where /.flatpak-info is), found once.
static gboolean sandboxed = FALSE;

// Fogata: the files the FileTransfer portal holds under `key`, as a text/uri-list of the
// paths this application can open (the document portal's, or the file's own where the
// sandbox sees it already), or null. Asked while the drop is still going: a source may end
// its transfer once the drop is finished. These paths do not say where a file really is:
// the Dart part asks the Documents portal that (documentHostPaths) before it takes one.
static gchar *portal_uri_list(const gchar *key) {
  // The key is the source's data: the portal is asked only with text a D-Bus string can be.
  if (*key == '\0' || !g_utf8_validate(key, -1, nullptr)) {
    return nullptr;
  }
  g_autoptr(GError) error = nullptr;
  g_autoptr(GDBusConnection) bus = g_bus_get_sync(G_BUS_TYPE_SESSION, nullptr, &error);
  if (bus == nullptr) {
    g_warning("desktop_drop: no session bus: %s", error->message);
    return nullptr;
  }
  g_autoptr(GVariant) reply = g_dbus_connection_call_sync(
      bus, "org.freedesktop.portal.Documents", "/org/freedesktop/portal/documents",
      "org.freedesktop.portal.FileTransfer", "RetrieveFiles",
      g_variant_new("(s@a{sv})", key,
                    g_variant_new_array(G_VARIANT_TYPE("{sv}"), nullptr, 0)),
      G_VARIANT_TYPE("(as)"), G_DBUS_CALL_FLAGS_NONE, 5000, nullptr, &error);
  if (reply == nullptr) {
    g_warning("desktop_drop: the portal gave no files: %s", error->message);
    return nullptr;
  }
  g_auto(GStrv) paths = nullptr;
  g_variant_get(reply, "(^as)", &paths);
  g_autoptr(GString) list = g_string_new(nullptr);
  for (gchar **path = paths; path != nullptr && *path != nullptr; path++) {
    g_autofree gchar *uri = g_filename_to_uri(*path, nullptr, nullptr);
    if (uri != nullptr) {
      g_string_append(list, uri);
      g_string_append(list, "\r\n");
    }
  }
  if (list->len == 0) {
    return nullptr;
  }
  return g_string_free(static_cast<GString *>(g_steal_pointer(&list)), FALSE);
}

void on_drag_data_received(GtkWidget *widget, GdkDragContext *drag_context,
                           gint x, gint y, GtkSelectionData *sdata, guint info,
                           guint time, gpointer user_data) {
  auto *channel = static_cast<FlMethodChannel *>(user_data);
  // Fogata: only a text/uri-list is taken, or inside the Flatpak the portal's key to a list
  // of files (see the targets below), and nothing when the source gave no data.
  const GdkAtom target = gtk_selection_data_get_target(sdata);
  const gboolean portal =
      sandboxed && (target == gdk_atom_intern_static_string(kPortalTarget) ||
                    target == gdk_atom_intern_static_string(kPortalFilesTarget));
  if ((!portal && target != gdk_atom_intern_static_string("text/uri-list")) ||
      gtk_selection_data_get_length(sdata) < 0) {
    return;
  }
  g_autofree gchar *data =
      g_strndup(reinterpret_cast<const gchar *>(gtk_selection_data_get_data(sdata)),
                gtk_selection_data_get_length(sdata));
  if (portal) {
    // The portal's files, handed on as the list of files they are. A portal that gives
    // nothing drops nothing.
    g_autofree gchar *list = portal_uri_list(g_strstrip(data));
    if (list == nullptr) {
      return;
    }
    g_free(data);
    data = static_cast<gchar *>(g_steal_pointer(&list));
  }
  double point[] = {double(x), double(y)};
  // Fogata: the list and what it holds freed once sent (fl_value_append_take hands each
  // value to the list, which fl_value_append would leave with a reference of its own).
  g_autoptr(FlValue) args = fl_value_new_list();
  fl_value_append_take(args, fl_value_new_string(data));
  fl_value_append_take(args, fl_value_new_float_list(point, 2));
  // Fogata: what the data is given as, which the Dart part checks again: the portal's
  // files are a list of files too.
  g_autofree gchar *given = portal ? g_strdup("text/uri-list") : gdk_atom_name(target);
  fl_value_append_take(args, fl_value_new_string(given));
  fl_method_channel_invoke_method(channel, "performOperation_linux", args,
                                  nullptr, nullptr, nullptr);
}

void on_drag_motion(GtkWidget *widget, GdkDragContext *drag_context,
                    gint x, gint y, guint time, gpointer user_data) {
  if (ignoreNext) {
    ignoreNext = FALSE;
    return;
  }

  // Fogata: a drag that offers no file list (text from another application) is not
  // shown as something that can be dropped.
  if (gtk_drag_dest_find_target(widget, drag_context, nullptr) == GDK_NONE) {
    return;
  }

  auto *channel = static_cast<FlMethodChannel *>(user_data);
  double point[] = {double(x), double(y)};
  g_autoptr(FlValue) args = fl_value_new_float_list(point, 2);
  fl_method_channel_invoke_method(channel, "updated", args,
                                  nullptr, nullptr, nullptr);
}

void on_drag_leave(GtkWidget *widget, GdkDragContext *drag_context, guint time, gpointer user_data) {
  auto *channel = static_cast<FlMethodChannel *>(user_data);
  fl_method_channel_invoke_method(channel, "exited", nullptr,
                                  nullptr, nullptr, nullptr);
}

// Fogata: answers documentHostPaths with what the Documents portal answered: each id it
// knows, and this application was given, with the file's real path as bytes. An id it does
// not answer for is left out, and so is every id when it cannot be asked or is older than
// GetHostPaths (version 5 of its interface): the Dart part then takes none of those files.
static void on_host_paths(GObject *source, GAsyncResult *result, gpointer user_data) {
  g_autoptr(FlMethodCall) method_call = FL_METHOD_CALL(user_data);
  g_autoptr(GError) error = nullptr;
  g_autoptr(GVariant) reply =
      g_dbus_connection_call_finish(G_DBUS_CONNECTION(source), result, &error);
  g_autoptr(FlValue) found = fl_value_new_map();
  if (reply == nullptr) {
    g_warning("desktop_drop: the portal gave no real places: %s", error->message);
  } else {
    g_autoptr(GVariantIter) iter = nullptr;
    g_variant_get(reply, "(a{say})", &iter);
    const gchar *id = nullptr;
    GVariant *path = nullptr;
    while (g_variant_iter_loop(iter, "{&s@ay}", &id, &path)) {
      // A byte string ends in a NUL, which is not part of the path.
      const gchar *bytes = g_variant_get_bytestring(path);
      fl_value_set_string_take(
          found, id,
          fl_value_new_uint8_list(reinterpret_cast<const uint8_t *>(bytes), strlen(bytes)));
    }
  }
  fl_method_call_respond_success(method_call, found, nullptr);
}

// Called when a method call is received from Flutter.
static void desktop_drop_plugin_handle_method_call(
    DesktopDropPlugin *self,
    FlMethodCall *method_call) {
  // Fogata: where the document portal's files with the ids given really are, which only
  // the portal can say (GetHostPaths), never the drag's own data.
  if (strcmp(fl_method_call_get_name(method_call), "documentHostPaths") == 0) {
    FlValue *args = fl_method_call_get_args(method_call);
    if (args == nullptr || fl_value_get_type(args) != FL_VALUE_TYPE_LIST) {
      fl_method_call_respond_error(method_call, "bad-args", "a list of ids", nullptr,
                                   nullptr);
      return;
    }
    g_autoptr(GError) error = nullptr;
    g_autoptr(GDBusConnection) bus = g_bus_get_sync(G_BUS_TYPE_SESSION, nullptr, &error);
    if (bus == nullptr) {
      g_warning("desktop_drop: no session bus: %s", error->message);
      g_autoptr(FlValue) none = fl_value_new_map();
      fl_method_call_respond_success(method_call, none, nullptr);
      return;
    }
    GVariantBuilder ids;
    g_variant_builder_init(&ids, G_VARIANT_TYPE("as"));
    for (size_t i = 0; i < fl_value_get_length(args); i++) {
      FlValue *id = fl_value_get_list_value(args, i);
      if (fl_value_get_type(id) == FL_VALUE_TYPE_STRING) {
        g_variant_builder_add(&ids, "s", fl_value_get_string(id));
      }
    }
    g_dbus_connection_call(
        bus, "org.freedesktop.portal.Documents", "/org/freedesktop/portal/documents",
        "org.freedesktop.portal.Documents", "GetHostPaths", g_variant_new("(as)", &ids),
        G_VARIANT_TYPE("(a{say})"), G_DBUS_CALL_FLAGS_NONE, 5000, nullptr, on_host_paths,
        g_object_ref(method_call));
    return;
  }
  fl_method_call_respond_not_implemented(method_call, nullptr);
}

static void desktop_drop_plugin_dispose(GObject *object) {
  G_OBJECT_CLASS(desktop_drop_plugin_parent_class)->dispose(object);
}

static void desktop_drop_plugin_class_init(DesktopDropPluginClass *klass) {
  G_OBJECT_CLASS(klass)->dispose = desktop_drop_plugin_dispose;
}

static void desktop_drop_plugin_init(DesktopDropPlugin *self) {
  const char * desktopEnv = getenv("XDG_CURRENT_DESKTOP");
  if (desktopEnv) {
    const char * lowercaseDesktopEnv = g_ascii_strdown(desktopEnv, -1);

    if (strcmp(lowercaseDesktopEnv, "kde") == 0 || strcmp(lowercaseDesktopEnv, "plasma") == 0) {
        isKDE = TRUE;
    }
  }
}

static gboolean on_focus_in_event(GtkWidget *widget, GdkEventFocus *event, gpointer user_data) {
  if (isKDE) {
    ignoreNext = TRUE;
  }
  return FALSE;
}

static void method_call_cb(FlMethodChannel *channel, FlMethodCall *method_call,
                           gpointer user_data) {
  DesktopDropPlugin *plugin = DESKTOP_DROP_PLUGIN(user_data);
  desktop_drop_plugin_handle_method_call(plugin, method_call);
}

void desktop_drop_plugin_register_with_registrar(FlPluginRegistrar *registrar) {
  DesktopDropPlugin *plugin = DESKTOP_DROP_PLUGIN(
      g_object_new(desktop_drop_plugin_get_type(), nullptr));

  auto *fl_view = fl_plugin_registrar_get_view(registrar);
  // Fogata: files only (text/uri-list). The plain text target (STRING) is not taken: any
  // application, a web page among them, could drop text that reads as a path. Inside the
  // Flatpak, which sees none of the computer's files but the downloads folder, the
  // portal's targets come first: a source that offers them beside the list (GTK and KDE
  // applications do) hands its files to the sandbox through the portal, where the list's
  // paths could not be opened. GTK takes the first of these targets the source offers.
  // Outside the Flatpak the targets are as before: the list, then the portal's targets,
  // which GTK adds where a portal runs and which drop nothing there.
  static GtkTargetEntry portal_targets[] = {
      {const_cast<gchar *>(kPortalTarget), GTK_TARGET_OTHER_APP, 0},
      {const_cast<gchar *>(kPortalFilesTarget), GTK_TARGET_OTHER_APP, 0}};
  sandboxed = g_file_test("/.flatpak-info", G_FILE_TEST_EXISTS);
  gtk_drag_dest_set(GTK_WIDGET(fl_view), GTK_DEST_DEFAULT_ALL,
                    sandboxed ? portal_targets : nullptr,
                    sandboxed ? G_N_ELEMENTS(portal_targets) : 0, GDK_ACTION_COPY);
  gtk_drag_dest_add_uri_targets(GTK_WIDGET(fl_view));

  g_autoptr(FlStandardMethodCodec) codec = fl_standard_method_codec_new();
  FlMethodChannel *channel =
      fl_method_channel_new(fl_plugin_registrar_get_messenger(registrar),
                            "desktop_drop",
                            FL_METHOD_CODEC(codec));
  fl_method_channel_set_method_call_handler(channel, method_call_cb,
                                            g_object_ref(plugin),
                                            g_object_unref);

  g_signal_connect(fl_view, "drag-motion",
                   G_CALLBACK(on_drag_motion), channel);
  g_signal_connect(GTK_WIDGET(fl_view), "drag-data-received",
                   G_CALLBACK(on_drag_data_received), channel);
  g_signal_connect(GTK_WIDGET(fl_view), "drag-leave",
                   G_CALLBACK(on_drag_leave), channel);
  g_signal_connect(fl_view, "focus-in-event",
                   G_CALLBACK(on_focus_in_event), nullptr);

  g_object_unref(plugin);
}
