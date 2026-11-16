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

// Fogata: how long the FileTransfer portal is waited for: the drop is answered meanwhile.
static const gint kPortalWaitMs = 2000;

// Fogata: hands a drop to the Dart part: its text (a text/uri-list), where it was dropped,
// what the text was given as, and inside the Flatpak, when the portal's files could not be
// taken, why ("declined": the list named a file the Dart part refuses, so the portal was
// not asked; "unreached": the portal gave nothing in time) and of how many files.
static void send_drop(FlMethodChannel *channel, const gchar *text, gint x, gint y,
                      const gchar *target, const gchar *status, gint64 count) {
  double point[] = {double(x), double(y)};
  // Fogata: the list and what it holds freed once sent (fl_value_append_take hands each
  // value to the list, which fl_value_append would leave with a reference of its own).
  g_autoptr(FlValue) args = fl_value_new_list();
  fl_value_append_take(args, fl_value_new_string(text));
  fl_value_append_take(args, fl_value_new_float_list(point, 2));
  fl_value_append_take(args, fl_value_new_string(target));
  if (status != nullptr) {
    fl_value_append_take(args, fl_value_new_string(status));
    fl_value_append_take(args, fl_value_new_int(count));
  }
  fl_method_channel_invoke_method(channel, "performOperation_linux", args,
                                  nullptr, nullptr, nullptr);
}

// Fogata: a drop inside the Flatpak whose source offers the portal's target, from its
// "drag-drop" to gtk_drag_finish, which waits for the Dart part and the portal. A file of
// the portal's transfer is exported to this application for good when its files are
// retrieved (it cannot give a document back), so when the source also offers the list of
// files, the list is read first and judged by the Dart part (checkPortalDrop, names
// only: the rules are there), and the portal is asked only when every file passes.
typedef struct {
  guint serial;
  GtkWidget *widget;        // a reference
  GdkDragContext *context;  // a reference
  FlMethodChannel *channel;
  gint x, y;
  guint time;
  GdkAtom portal;  // the portal's target the source offers
  gchar *list;     // the list's text, once read
} PortalDrop;

static void portal_drop_free(PortalDrop *drop) {
  g_object_unref(drop->widget);
  g_object_unref(drop->context);
  g_free(drop->list);
  g_free(drop);
}
G_DEFINE_AUTOPTR_CLEANUP_FUNC(PortalDrop, portal_drop_free)

static PortalDrop *portal_drop = nullptr;
static guint portal_serial = 0;

// The drop [serial] names, if it is still the one going.
static PortalDrop *current_drop(gpointer serial) {
  return portal_drop != nullptr && portal_drop->serial == GPOINTER_TO_UINT(serial)
             ? portal_drop
             : nullptr;
}

// How many files of this computer the drop's list named, or 1 when it was not read.
static gint64 files_named(const PortalDrop *drop) {
  if (drop->list == nullptr) {
    return 1;
  }
  g_auto(GStrv) uris = g_uri_list_extract_uris(drop->list);
  gint64 files = 0;
  for (gchar **uri = uris; uri != nullptr && *uri != nullptr; uri++) {
    if (g_str_has_prefix(*uri, "file:")) {
      files++;
    }
  }
  return files > 0 ? files : 1;
}

// Ends the drop going: finished as taken when [text] holds its files, else refused, with
// [status] and [count] for the Dart part.
static void end_portal_drop(const gchar *text, const gchar *status, gint64 count) {
  g_autoptr(PortalDrop) drop = portal_drop;
  portal_drop = nullptr;
  gtk_drag_finish(drop->context, text != nullptr, FALSE, drop->time);
  if (text != nullptr || status != nullptr) {
    send_drop(drop->channel, text != nullptr ? text : "", drop->x, drop->y,
              "text/uri-list", status, count);
  }
}

// The portal's answer: its files, handed on as the list of files they are (the document
// portal's paths, or the file's own where the sandbox sees it already). These paths do not
// say where a file really is: the Dart part asks the Documents portal that
// (documentHostPaths) before it takes one. A portal that gives nothing in time says so.
static void on_retrieved(GObject *source, GAsyncResult *result, gpointer serial) {
  g_autoptr(GError) error = nullptr;
  g_autoptr(GVariant) reply =
      g_dbus_connection_call_finish(G_DBUS_CONNECTION(source), result, &error);
  PortalDrop *drop = current_drop(serial);
  if (drop == nullptr) {
    return;
  }
  if (reply == nullptr) {
    g_warning("desktop_drop: the portal gave no files: %s", error->message);
    end_portal_drop(nullptr, "unreached", files_named(drop));
    return;
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
    end_portal_drop(nullptr, "unreached", files_named(drop));
    return;
  }
  end_portal_drop(list->str, nullptr, 0);
}

// Asks the FileTransfer portal for the files under [key], the source's data, while the
// drop is still going: a source may end its transfer once the drop is finished.
static void retrieve(PortalDrop *drop, const gchar *key) {
  // The portal is asked only with text a D-Bus string can be.
  if (*key == '\0' || !g_utf8_validate(key, -1, nullptr)) {
    end_portal_drop(nullptr, "unreached", files_named(drop));
    return;
  }
  g_autoptr(GError) error = nullptr;
  g_autoptr(GDBusConnection) bus = g_bus_get_sync(G_BUS_TYPE_SESSION, nullptr, &error);
  if (bus == nullptr) {
    g_warning("desktop_drop: no session bus: %s", error->message);
    end_portal_drop(nullptr, "unreached", files_named(drop));
    return;
  }
  g_dbus_connection_call(
      bus, "org.freedesktop.portal.Documents", "/org/freedesktop/portal/documents",
      "org.freedesktop.portal.FileTransfer", "RetrieveFiles",
      g_variant_new("(s@a{sv})", key,
                    g_variant_new_array(G_VARIANT_TYPE("{sv}"), nullptr, 0)),
      G_VARIANT_TYPE("(as)"), G_DBUS_CALL_FLAGS_NONE, kPortalWaitMs, nullptr, on_retrieved,
      GUINT_TO_POINTER(drop->serial));
}

// The Dart part's judgement of the list: how many of its files it refuses. Any other
// answer, or none, refuses the drop too: the portal is asked only when it says 0.
static void on_checked(GObject *source, GAsyncResult *result, gpointer serial) {
  g_autoptr(GError) error = nullptr;
  g_autoptr(FlMethodResponse) response =
      fl_method_channel_invoke_method_finish(FL_METHOD_CHANNEL(source), result, &error);
  PortalDrop *drop = current_drop(serial);
  if (drop == nullptr) {
    return;
  }
  gint64 refused = -1;
  if (response != nullptr) {
    FlValue *value = fl_method_response_get_result(response, nullptr);
    if (value != nullptr && fl_value_get_type(value) == FL_VALUE_TYPE_INT) {
      refused = fl_value_get_int(value);
    }
  }
  if (refused != 0) {
    end_portal_drop(nullptr, "declined", refused > 0 ? refused : 0);
    return;
  }
  gtk_drag_get_data(drop->widget, drop->context, drop->portal, drop->time);
}

static gboolean is_portal(GdkAtom target) {
  return target == gdk_atom_intern_static_string(kPortalTarget) ||
         target == gdk_atom_intern_static_string(kPortalFilesTarget);
}

// Fogata: the drop itself, inside the Flatpak (outside it, GTK_DEST_DEFAULT_DROP takes it
// as published). A drop that offers no portal's target is taken as GTK would.
static gboolean on_drag_drop(GtkWidget *widget, GdkDragContext *context, gint x, gint y,
                             guint time, gpointer user_data) {
  const GdkAtom target = gtk_drag_dest_find_target(widget, context, nullptr);
  if (target == GDK_NONE) {
    return FALSE;
  }
  if (!is_portal(target)) {
    gtk_drag_get_data(widget, context, target, time);
    return TRUE;
  }
  if (portal_drop != nullptr) {
    // A drop still waited for is given up: its source has moved on.
    end_portal_drop(nullptr, nullptr, 0);
  }
  portal_drop = g_new0(PortalDrop, 1);
  portal_drop->serial = ++portal_serial;
  portal_drop->widget = GTK_WIDGET(g_object_ref(widget));
  portal_drop->context = GDK_DRAG_CONTEXT(g_object_ref(context));
  portal_drop->channel = static_cast<FlMethodChannel *>(user_data);
  portal_drop->x = x;
  portal_drop->y = y;
  portal_drop->time = time;
  portal_drop->portal = target;
  const GdkAtom list = gdk_atom_intern_static_string("text/uri-list");
  const gboolean offers_list =
      g_list_find(gdk_drag_context_list_targets(context), GDK_ATOM_TO_POINTER(list)) !=
      nullptr;
  gtk_drag_get_data(widget, context, offers_list ? list : target, time);
  return TRUE;
}

void on_drag_data_received(GtkWidget *widget, GdkDragContext *drag_context,
                           gint x, gint y, GtkSelectionData *sdata, guint info,
                           guint time, gpointer user_data) {
  auto *channel = static_cast<FlMethodChannel *>(user_data);
  const GdkAtom target = gtk_selection_data_get_target(sdata);
  const gboolean given = gtk_selection_data_get_length(sdata) >= 0;
  g_autofree gchar *data =
      given ? g_strndup(reinterpret_cast<const gchar *>(gtk_selection_data_get_data(sdata)),
                        gtk_selection_data_get_length(sdata))
            : nullptr;
  if (sandboxed) {
    PortalDrop *drop = portal_drop != nullptr && portal_drop->context == drag_context
                           ? portal_drop
                           : nullptr;
    if (drop == nullptr) {
      // Fogata: taken as GTK_DEST_DEFAULT_DROP takes it outside the Flatpak: a list of
      // files, and nothing when the source gave no data.
      if (given && target == gdk_atom_intern_static_string("text/uri-list")) {
        g_autofree gchar *name = gdk_atom_name(target);
        send_drop(channel, data, x, y, name, nullptr, 0);
      }
      gtk_drag_finish(drag_context, given, FALSE, time);
      return;
    }
    if (!given) {
      end_portal_drop(nullptr, "unreached", files_named(drop));
    } else if (!is_portal(target) && drop->list == nullptr) {
      // The list first, judged by the Dart part before the portal is asked.
      drop->list = static_cast<gchar *>(g_steal_pointer(&data));
      g_autoptr(FlValue) args = fl_value_new_list();
      fl_value_append_take(args, fl_value_new_string(drop->list));
      fl_method_channel_invoke_method(channel, "checkPortalDrop", args, nullptr, on_checked,
                                      GUINT_TO_POINTER(drop->serial));
    } else if (is_portal(target)) {
      retrieve(drop, g_strstrip(data));
    } else {
      end_portal_drop(nullptr, "unreached", files_named(drop));
    }
    return;
  }
  // Fogata: only a text/uri-list is taken (see the targets below), and nothing when the
  // source gave no data.
  if (target != gdk_atom_intern_static_string("text/uri-list") || !given) {
    return;
  }
  // Fogata: what the data was given as, which the Dart part checks again.
  g_autofree gchar *name = gdk_atom_name(target);
  send_drop(channel, data, x, y, name, nullptr, 0);
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
// knows, and this application was given, with the file's real path as bytes; an id it does
// not answer for is left out. When it cannot be asked, gives no answer in time or is older
// than GetHostPaths (version 5 of its interface), an error: the Dart part then takes none
// of those files, and says that their place cannot be learnt here.
static void on_host_paths(GObject *source, GAsyncResult *result, gpointer user_data) {
  g_autoptr(FlMethodCall) method_call = FL_METHOD_CALL(user_data);
  g_autoptr(GError) error = nullptr;
  g_autoptr(GVariant) reply =
      g_dbus_connection_call_finish(G_DBUS_CONNECTION(source), result, &error);
  if (reply == nullptr) {
    g_warning("desktop_drop: the portal gave no real places: %s", error->message);
    fl_method_call_respond_error(method_call, "portal", error->message, nullptr, nullptr);
    return;
  }
  g_autoptr(FlValue) found = fl_value_new_map();
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
      fl_method_call_respond_error(method_call, "portal", error->message, nullptr, nullptr);
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
        G_VARIANT_TYPE("(a{say})"), G_DBUS_CALL_FLAGS_NONE, kPortalWaitMs, nullptr,
        on_host_paths,
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
  // paths could not be opened. GTK takes the first of these targets the source offers, and
  // the drop is taken by hand (on_drag_drop): the list is judged before the portal is
  // asked. Outside the Flatpak the targets and the drop are as before: the list, then the
  // portal's targets, which GTK adds where a portal runs and which drop nothing there.
  static GtkTargetEntry portal_targets[] = {
      {const_cast<gchar *>(kPortalTarget), GTK_TARGET_OTHER_APP, 0},
      {const_cast<gchar *>(kPortalFilesTarget), GTK_TARGET_OTHER_APP, 0}};
  sandboxed = g_file_test("/.flatpak-info", G_FILE_TEST_EXISTS);
  gtk_drag_dest_set(GTK_WIDGET(fl_view),
                    sandboxed ? static_cast<GtkDestDefaults>(GTK_DEST_DEFAULT_MOTION |
                                                             GTK_DEST_DEFAULT_HIGHLIGHT)
                              : GTK_DEST_DEFAULT_ALL,
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
  if (sandboxed) {
    g_signal_connect(GTK_WIDGET(fl_view), "drag-drop", G_CALLBACK(on_drag_drop), channel);
  }
  g_signal_connect(fl_view, "focus-in-event",
                   G_CALLBACK(on_focus_in_event), nullptr);

  g_object_unref(plugin);
}
