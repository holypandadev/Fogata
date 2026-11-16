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

// Fogata: how long a drop inside the Flatpak may take before the FileTransfer portal is
// asked (its list read and judged by the Dart part), how long the portal is waited for once
// asked (its answer is then taken, or judged, whenever it comes within that), and how long
// the Documents portal is waited for. The drop is answered meanwhile.
static const guint kBeforePortalMs = 5000;
static const gint kPortalWaitMs = 20000;
static const gint kHostPathsWaitMs = 5000;

// Fogata: the session bus, got once without holding up the window (inside the Flatpak
// only); null until then, or when there is none.
static GDBusConnection *session_bus = nullptr;

static void on_session_bus(GObject *source, GAsyncResult *result, gpointer user_data) {
  g_autoptr(GError) error = nullptr;
  session_bus = g_bus_get_finish(result, &error);
  if (session_bus == nullptr) {
    g_warning("desktop_drop: no session bus: %s", error->message);
  }
}

// Fogata: hands a drop to the Dart part: its text (a text/uri-list), where it was dropped,
// what the text was given as, and inside the Flatpak, when the portal's files were not
// taken, why ("declined": the list named a file the Dart part refuses, so the portal was
// not asked; "unreached": no file could be taken, the portal was not asked or gave
// nothing) and of how many files.
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

// Fogata: a drop inside the Flatpak, taken by hand from its "drag-drop" to gtk_drag_finish
// (outside it GTK_DEST_DEFAULT_ALL takes drops as published). A file of the FileTransfer
// portal's transfer is exported to this application for good when its files are retrieved
// (it can neither give a document back nor delete it), so the portal is asked only for a
// drop whose list of files the Dart part has read and judged (checkPortalDrop: every entry
// a plain file of this computer, by its name, as GIO reads it too, where a drop target
// that takes it is); a source that offers no list is never asked. The guarantee rests on
// the source building both targets from one list, as GTK's gtk_selection_data_set_uris
// does (with the GIO calls the plugin makes); a transfer that gives another number of files
// than the list named is not taken, though the portal has exported them by then.
//
// One drop at a time, and GDK keeps one drop context a seat on Wayland: a new drag that
// enters the window takes it over, its offer in place of the dropped one's (GDK cancels a
// read still going for the old one, with a reply of no data). So every drag that enters
// starts a new generation (on_drag_motion), which ends a drop still going without finishing
// it, and nothing is done to the drag context (gtk_drag_finish, gtk_drag_get_data) for a
// drop whose generation has passed. Those steps are taken from an idle callback of low
// priority, after GDK has handled the events already come (a new drag's enter among them),
// and only for data of the target the drop going asked for (and, where the time tells, as
// on X11, of its own time: on Wayland every time is 0).
typedef struct {
  guint serial;
  guint generation;  // of the drag that dropped it
  GtkWidget *widget;        // a reference
  GdkDragContext *context;  // a reference
  FlMethodChannel *channel;
  gint x, y;
  guint time;
  GdkAtom portal;    // the portal's target the source offers, or GDK_NONE
  GdkAtom awaiting;  // what was asked for and has not come, or GDK_NONE
  gchar *list;       // the list's text, once read
  guint entries;     // how many entries of the list were judged
  guint deadline;    // the timeout before the portal is asked (or a plain drop's data
                     // comes), 0 once gone
  guint idle;        // the step waiting for GDK's events, 0 when none
  // The step: finish the drag and hand on what it brought, or ask for the portal's target.
  gboolean ask_portal;
  gboolean success;
  gchar *text;
  const gchar *target;
  const gchar *status;
  gint64 count;
} Drop;

static void drop_free(Drop *drop) {
  if (drop->deadline != 0) {
    g_source_remove(drop->deadline);
  }
  if (drop->idle != 0) {
    g_source_remove(drop->idle);
  }
  g_object_unref(drop->widget);
  g_object_unref(drop->context);
  g_free(drop->list);
  g_free(drop->text);
  g_free(drop);
}
G_DEFINE_AUTOPTR_CLEANUP_FUNC(Drop, drop_free)

static Drop *current_drop = nullptr;
static guint drop_serial = 0;
// Fogata: the drag over the window now: one more for each drag that enters it.
static guint drag_generation = 0;
static gboolean drag_inside = FALSE;

// The drop [serial] names, if it is still the one going.
static Drop *drop_of(gpointer serial) {
  return current_drop != nullptr && current_drop->serial == GPOINTER_TO_UINT(serial)
             ? current_drop
             : nullptr;
}

// How many files of this computer the list named, each once, or 1 when there is none.
static gint64 files_named(const gchar *list) {
  if (list == nullptr) {
    return 1;
  }
  g_auto(GStrv) uris = g_uri_list_extract_uris(list);
  g_autoptr(GHashTable) files = g_hash_table_new(g_str_hash, g_str_equal);
  for (gchar **uri = uris; uri != nullptr && *uri != nullptr; uri++) {
    if (g_ascii_strncasecmp(*uri, "file:", 5) == 0) {
      g_hash_table_add(files, *uri);
    }
  }
  const guint named = g_hash_table_size(files);
  return named > 0 ? named : 1;
}

// Ends the drop going: hands on [text] (or [status] and [count]) to the Dart part, unless
// neither is given, and finishes the drag when [finish], which is only done from step()
// for a drop of the drag that is still the current one. What is reported back to the
// source does not depend on whether the files could be taken (a page could learn from it
// whether a file it named is there): a drop that offered the portal's target is reported
// taken, any other as GTK reports it, by whether its data came.
static void end_drop(gboolean finish, gboolean success, const gchar *text,
                     const gchar *target, const gchar *status, gint64 count) {
  g_autoptr(Drop) drop = current_drop;
  current_drop = nullptr;
  if (finish) {
    gtk_drag_finish(drop->context, drop->portal != GDK_NONE ? TRUE : success, FALSE,
                    drop->time);
  }
  if (text != nullptr || status != nullptr) {
    send_drop(drop->channel, text != nullptr ? text : "", drop->x, drop->y, target,
              status, count);
  }
}

// The step waiting for GDK's events: taken only while no other drag has entered since the
// drop. Else the drop is given up, finishing nothing, and only files the portal gave are
// still handed on (to be judged: they were exported already).
static gboolean step(gpointer serial) {
  Drop *drop = drop_of(serial);
  if (drop == nullptr) {
    return G_SOURCE_REMOVE;
  }
  drop->idle = 0;
  const gboolean current = drop->generation == drag_generation;
  if (drop->ask_portal) {
    drop->ask_portal = FALSE;
    if (!current) {
      end_drop(FALSE, FALSE, nullptr, nullptr, nullptr, 0);
      return G_SOURCE_REMOVE;
    }
    drop->awaiting = drop->portal;
    gtk_drag_get_data(drop->widget, drop->context, drop->portal, drop->time);
    return G_SOURCE_REMOVE;
  }
  g_autofree gchar *text = static_cast<gchar *>(g_steal_pointer(&drop->text));
  if (!current && text == nullptr) {
    end_drop(FALSE, FALSE, nullptr, nullptr, nullptr, 0);
  } else {
    end_drop(current, drop->success, text, drop->target, drop->status, drop->count);
  }
  return G_SOURCE_REMOVE;
}

// Waits for GDK's events before the drop's next step (see step()).
static void later(Drop *drop) {
  if (drop->idle == 0) {
    drop->idle = g_idle_add_full(G_PRIORITY_LOW, step, GUINT_TO_POINTER(drop->serial),
                                 nullptr);
  }
}

// The drop's end, once GDK's events are handled.
static void end_later(Drop *drop, gboolean success, const gchar *text, const gchar *target,
                      const gchar *status, gint64 count) {
  if (drop->deadline != 0) {
    g_source_remove(drop->deadline);
    drop->deadline = 0;
  }
  drop->awaiting = GDK_NONE;
  drop->success = success;
  g_free(drop->text);
  drop->text = g_strdup(text);
  drop->target = target;
  drop->status = status;
  drop->count = count;
  later(drop);
}

// The drop took too long before the portal was asked (a list that never came, a Dart part
// that never answered), or its data never came: nothing was exported.
static gboolean on_deadline(gpointer serial) {
  Drop *drop = drop_of(serial);
  if (drop != nullptr) {
    drop->deadline = 0;
    end_later(drop, FALSE, nullptr, "text/uri-list", "unreached", files_named(drop->list));
  }
  return G_SOURCE_REMOVE;
}

// What a RetrieveFiles call keeps of its drop, which may be given up before the portal
// answers: the files it gives are still handed on to be judged, since they were exported.
typedef struct {
  guint serial;
  FlMethodChannel *channel;
  gint x, y;
  guint entries;
  gint64 named;
} Retrieval;

// The portal's answer: its files, handed on as the list of files they are (the document
// portal's paths, or the file's own where the sandbox sees it already). These paths do not
// say where a file really is: the Dart part asks the Documents portal that
// (documentHostPaths) before it takes one. A portal that gives nothing, or another number
// of files than the list it was judged by named, gives nothing to take.
static void on_retrieved(GObject *source, GAsyncResult *result, gpointer user_data) {
  g_autofree Retrieval *retrieval = static_cast<Retrieval *>(user_data);
  g_autoptr(GError) error = nullptr;
  g_autoptr(GVariant) reply =
      g_dbus_connection_call_finish(G_DBUS_CONNECTION(source), result, &error);
  g_auto(GStrv) paths = nullptr;
  if (reply == nullptr) {
    g_warning("desktop_drop: the portal gave no files: %s", error->message);
  } else {
    g_variant_get(reply, "(^as)", &paths);
  }
  g_autoptr(GString) list = g_string_new(nullptr);
  if (paths != nullptr && g_strv_length(paths) == retrieval->entries) {
    for (gchar **path = paths; *path != nullptr; path++) {
      g_autofree gchar *uri = g_filename_to_uri(*path, nullptr, nullptr);
      if (uri != nullptr) {
        g_string_append(list, uri);
        g_string_append(list, "\r\n");
      }
    }
  } else if (paths != nullptr) {
    g_warning("desktop_drop: the portal gave %u files for a list of %u",
              g_strv_length(paths), retrieval->entries);
  }
  const gboolean given = list->len > 0;
  Drop *drop = drop_of(GUINT_TO_POINTER(retrieval->serial));
  if (drop == nullptr) {
    // Given up meanwhile (another drag came): what the portal gave is still judged.
    if (given) {
      send_drop(retrieval->channel, list->str, retrieval->x, retrieval->y, "text/uri-list",
                nullptr, 0);
    }
    return;
  }
  if (given) {
    end_later(drop, TRUE, list->str, "text/uri-list", nullptr, 0);
  } else {
    end_later(drop, FALSE, nullptr, "text/uri-list", "unreached", retrieval->named);
  }
}

// Asks the FileTransfer portal for the files under [key], the source's data, while the
// drop is still going: a source may end its transfer once the drop is finished. From here
// the drop waits for the portal's answer alone, which is taken (or judged) whenever it
// comes within kPortalWaitMs, even when the drop was given up meanwhile.
static void retrieve(Drop *drop, const gchar *key) {
  if (drop->deadline != 0) {
    g_source_remove(drop->deadline);
    drop->deadline = 0;
  }
  // The portal is asked only with text a D-Bus string can be.
  if (*key == '\0' || !g_utf8_validate(key, -1, nullptr) || session_bus == nullptr) {
    end_later(drop, FALSE, nullptr, "text/uri-list", "unreached", files_named(drop->list));
    return;
  }
  auto *retrieval = g_new0(Retrieval, 1);
  retrieval->serial = drop->serial;
  retrieval->channel = drop->channel;
  retrieval->x = drop->x;
  retrieval->y = drop->y;
  retrieval->entries = drop->entries;
  retrieval->named = files_named(drop->list);
  g_dbus_connection_call(
      session_bus, "org.freedesktop.portal.Documents", "/org/freedesktop/portal/documents",
      "org.freedesktop.portal.FileTransfer", "RetrieveFiles",
      g_variant_new("(s@a{sv})", key,
                    g_variant_new_array(G_VARIANT_TYPE("{sv}"), nullptr, 0)),
      G_VARIANT_TYPE("(as)"), G_DBUS_CALL_FLAGS_NONE, kPortalWaitMs, nullptr, on_retrieved,
      retrieval);
}

// The Dart part's judgement of the drop: how many files of its list it refuses (then the
// portal is not asked), 0 to ask the portal, -1 when it could not judge the list (no list,
// a line that is no plain file of this computer, none at all, no judge), -2 when no drop
// target takes a drop there. Any other answer, or none, is taken as -1.
static void on_checked(GObject *source, GAsyncResult *result, gpointer serial) {
  g_autoptr(GError) error = nullptr;
  g_autoptr(FlMethodResponse) response =
      fl_method_channel_invoke_method_finish(FL_METHOD_CHANNEL(source), result, &error);
  Drop *drop = drop_of(serial);
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
  if (refused == -2) {
    end_later(drop, FALSE, nullptr, nullptr, nullptr, 0);
  } else if (refused > 0) {
    end_later(drop, FALSE, nullptr, "text/uri-list", "declined", refused);
  } else if (refused != 0) {
    end_later(drop, FALSE, nullptr, "text/uri-list", "unreached", files_named(drop->list));
  } else {
    drop->ask_portal = TRUE;
    later(drop);
  }
}

// Asks the Dart part about the drop going: where it is, its list's text (null when the
// source offers none) and the paths GIO reads in that list's `file:` entries, as the
// source's GIO does when it hands the files to the portal ("" for any other entry, which
// GIO is not asked about, and for one it gives no local path).
static void check(Drop *drop) {
  g_autoptr(FlValue) args = fl_value_new_list();
  double point[] = {double(drop->x), double(drop->y)};
  fl_value_append_take(args, fl_value_new_float_list(point, 2));
  g_autoptr(FlValue) read = fl_value_new_list();
  if (drop->list != nullptr) {
    fl_value_append_take(args, fl_value_new_string(drop->list));
    g_auto(GStrv) uris = g_uri_list_extract_uris(drop->list);
    for (gchar **uri = uris; uri != nullptr && *uri != nullptr; uri++) {
      g_autofree gchar *path = nullptr;
      if (g_ascii_strncasecmp(*uri, "file:", 5) == 0) {
        g_autoptr(GFile) file = g_file_new_for_uri(*uri);
        path = g_file_get_path(file);
      }
      const gboolean text = path != nullptr && g_utf8_validate(path, -1, nullptr);
      fl_value_append_take(read, fl_value_new_string(text ? path : ""));
      drop->entries++;
    }
  } else {
    fl_value_append_take(args, fl_value_new_null());
  }
  fl_value_append(args, read);
  fl_method_channel_invoke_method(drop->channel, "checkPortalDrop", args, nullptr,
                                  on_checked, GUINT_TO_POINTER(drop->serial));
}

static gboolean is_portal(GdkAtom target) {
  return target == gdk_atom_intern_static_string(kPortalTarget) ||
         target == gdk_atom_intern_static_string(kPortalFilesTarget);
}

// Fogata: the drop itself, inside the Flatpak.
static gboolean on_drag_drop(GtkWidget *widget, GdkDragContext *context, gint x, gint y,
                             guint time, gpointer user_data) {
  if (current_drop != nullptr) {
    // A drop still waited for is given up, finishing nothing: on Wayland its drag context
    // is this drop's.
    end_drop(FALSE, FALSE, nullptr, nullptr, nullptr, 0);
  }
  const GdkAtom target = gtk_drag_dest_find_target(widget, context, nullptr);
  if (target == GDK_NONE) {
    // As GTK_DEST_DEFAULT_DROP does: a reply alone finishes no drop.
    gtk_drag_finish(context, FALSE, FALSE, time);
    return TRUE;
  }
  current_drop = g_new0(Drop, 1);
  current_drop->serial = ++drop_serial;
  current_drop->generation = drag_generation;
  // The drag is over (GTK says it has left first; this makes sure of it): the next one to
  // enter is another.
  drag_inside = FALSE;
  current_drop->widget = GTK_WIDGET(g_object_ref(widget));
  current_drop->context = GDK_DRAG_CONTEXT(g_object_ref(context));
  current_drop->channel = static_cast<FlMethodChannel *>(user_data);
  current_drop->x = x;
  current_drop->y = y;
  current_drop->time = time;
  current_drop->portal = is_portal(target) ? target : GDK_NONE;
  current_drop->deadline =
      g_timeout_add(kBeforePortalMs, on_deadline, GUINT_TO_POINTER(current_drop->serial));
  const GdkAtom list = gdk_atom_intern_static_string("text/uri-list");
  if (current_drop->portal == GDK_NONE) {
    current_drop->awaiting = target;
    gtk_drag_get_data(widget, context, target, time);
  } else if (g_list_find(gdk_drag_context_list_targets(context),
                         GDK_ATOM_TO_POINTER(list)) != nullptr) {
    current_drop->awaiting = list;
    gtk_drag_get_data(widget, context, list, time);
  } else {
    // No list to judge: the Dart part refuses it (and says so).
    check(current_drop);
  }
  return TRUE;
}

// Fogata: a window destroyed in the middle of a drop gives it up.
static void on_view_destroy(GtkWidget *widget, gpointer user_data) {
  if (current_drop != nullptr && current_drop->widget == widget) {
    end_drop(FALSE, FALSE, nullptr, nullptr, nullptr, 0);
  }
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
    Drop *drop = current_drop;
    if (drop == nullptr || drop->awaiting != target ||
        (time != GDK_CURRENT_TIME && drop->time != GDK_CURRENT_TIME && drop->time != time)) {
      // Not what the drop going asked for: a late answer to one given up.
      return;
    }
    drop->awaiting = GDK_NONE;
    if (drop->portal == GDK_NONE) {
      // Fogata: as GTK_DEST_DEFAULT_DROP takes it outside the Flatpak: a list of files,
      // and nothing when the source gave no data.
      g_autofree gchar *name = gdk_atom_name(target);
      const gboolean list = given && target == gdk_atom_intern_static_string("text/uri-list");
      end_later(drop, given, list ? data : nullptr, g_intern_string(name), nullptr, 0);
    } else if (!given) {
      // No data, which is also what GDK gives a read cancelled by another drag: once its
      // events are handled, this drop is given up if one came.
      end_later(drop, FALSE, nullptr, "text/uri-list", "unreached", files_named(drop->list));
    } else if (is_portal(target)) {
      retrieve(drop, g_strstrip(data));
    } else {
      // The list first, judged by the Dart part before the portal is asked.
      drop->list = static_cast<gchar *>(g_steal_pointer(&data));
      check(drop);
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
  // Fogata: inside the Flatpak, a drag that enters the window starts a new generation (see
  // Drop), and a drop still going is given up, finishing nothing: GDK has given its drag
  // context to the new drag.
  if (sandboxed && !drag_inside) {
    drag_inside = TRUE;
    drag_generation++;
    if (current_drop != nullptr) {
      end_drop(FALSE, FALSE, nullptr, nullptr, nullptr, 0);
    }
  }

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
  // Fogata: the drag has left the window, or dropped (GTK says so first).
  drag_inside = FALSE;
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
    if (session_bus == nullptr) {
      fl_method_call_respond_error(method_call, "portal", "no session bus", nullptr,
                                   nullptr);
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
        session_bus, "org.freedesktop.portal.Documents", "/org/freedesktop/portal/documents",
        "org.freedesktop.portal.Documents", "GetHostPaths", g_variant_new("(as)", &ids),
        G_VARIANT_TYPE("(a{say})"), G_DBUS_CALL_FLAGS_NONE, kHostPathsWaitMs, nullptr,
        on_host_paths, g_object_ref(method_call));
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
    g_signal_connect(GTK_WIDGET(fl_view), "destroy", G_CALLBACK(on_view_destroy), nullptr);
    // Every drag that enters is seen, one that offers nothing taken too (see Drop): on
    // Wayland it takes GDK's drop context over all the same.
    gtk_drag_dest_set_track_motion(GTK_WIDGET(fl_view), TRUE);
    g_bus_get(G_BUS_TYPE_SESSION, nullptr, on_session_bus, nullptr);
  }
  g_signal_connect(fl_view, "focus-in-event",
                   G_CALLBACK(on_focus_in_event), nullptr);

  g_object_unref(plugin);
}
