#include "desktop_channel.h"

#include <gio/gio.h>

#include <cstring>
#include <string>

namespace {

constexpr char kPortal[] = "org.freedesktop.portal.Desktop";
constexpr char kPortalPath[] = "/org/freedesktop/portal/desktop";

// Answers `call` with `value`, which it takes.
void respond(FlMethodCall* call, FlValue* value) {
  g_autoptr(FlValue) owned = value;
  g_autoptr(FlMethodResponse) response =
      FL_METHOD_RESPONSE(fl_method_success_response_new(owned));
  fl_method_call_respond(call, response, nullptr);
}

// ----- requestBackground -----

// One request to the Background portal, answered through its Request object's Response
// signal, which may come before the call's own reply: each of the two holds a reference.
struct Background {
  FlMethodCall* call = nullptr;
  GDBusConnection* bus = nullptr;
  GVariant* options = nullptr;
  std::string token;
  std::string path;
  guint subscription = 0;
  bool answered = false;
  int refs = 2;
};

void release(Background* b) {
  if (--b->refs > 0) return;
  g_object_unref(b->call);
  if (b->options != nullptr) g_variant_unref(b->options);
  if (b->bus != nullptr) g_object_unref(b->bus);
  delete b;
}

// Answers the application once, and lets the answer's reference go.
void finish(Background* b, FlValue* value) {
  if (b->answered) {
    fl_value_unref(value);
    return;
  }
  b->answered = true;
  if (b->subscription != 0) {
    g_dbus_connection_signal_unsubscribe(b->bus, b->subscription);
    b->subscription = 0;
  }
  respond(b->call, value);
  release(b);
}

void on_response(GDBusConnection*, const gchar*, const gchar*, const gchar*,
                 const gchar*, GVariant* parameters, gpointer data) {
  auto* b = static_cast<Background*>(data);
  guint32 response = 2;
  g_autoptr(GVariant) results = nullptr;
  g_variant_get(parameters, "(u@a{sv})", &response, &results);
  gboolean autostart = FALSE;
  if (response == 0 && results != nullptr) {
    g_variant_lookup(results, "autostart", "b", &autostart);
  }
  finish(b, fl_value_new_bool(response == 0 && autostart));
}

void listen(Background* b, const std::string& path) {
  if (b->subscription != 0) {
    g_dbus_connection_signal_unsubscribe(b->bus, b->subscription);
  }
  b->path = path;
  b->subscription = g_dbus_connection_signal_subscribe(
      b->bus, kPortal, "org.freedesktop.portal.Request", "Response",
      path.c_str(), nullptr, G_DBUS_SIGNAL_FLAGS_NO_MATCH_RULE, on_response, b,
      nullptr);
}

void on_requested(GObject* source, GAsyncResult* result, gpointer data) {
  auto* b = static_cast<Background*>(data);
  g_autoptr(GVariant) reply = g_dbus_connection_call_finish(
      G_DBUS_CONNECTION(source), result, nullptr);
  if (reply == nullptr) {
    finish(b, fl_value_new_null());
  } else if (!b->answered) {
    // Older portals answer on a path of their own.
    const gchar* handle = nullptr;
    g_variant_get(reply, "(&o)", &handle);
    if (handle != nullptr && b->path != handle) listen(b, handle);
  }
  release(b);
}

void request_background(FlMethodCall* call) {
  FlValue* args = fl_method_call_get_args(call);
  auto lookup = [args](const char* key, FlValueType type) -> FlValue* {
    if (fl_value_get_type(args) != FL_VALUE_TYPE_MAP) return nullptr;
    FlValue* v = fl_value_lookup_string(args, key);
    return v != nullptr && fl_value_get_type(v) == type ? v : nullptr;
  };
  FlValue* autostart = lookup("autostart", FL_VALUE_TYPE_BOOL);
  FlValue* commandline = lookup("commandline", FL_VALUE_TYPE_LIST);
  FlValue* reason = lookup("reason", FL_VALUE_TYPE_STRING);
  GVariantBuilder command;
  g_variant_builder_init(&command, G_VARIANT_TYPE_STRING_ARRAY);
  for (size_t i = 0;
       commandline != nullptr && i < fl_value_get_length(commandline); i++) {
    FlValue* part = fl_value_get_list_value(commandline, i);
    if (fl_value_get_type(part) == FL_VALUE_TYPE_STRING) {
      g_variant_builder_add(&command, "s", fl_value_get_string(part));
    }
  }
  static int next = 0;
  auto* b = new Background();
  b->call = FL_METHOD_CALL(g_object_ref(call));
  b->token = "fogata_background_" + std::to_string(++next);
  GVariantBuilder options;
  g_variant_builder_init(&options, G_VARIANT_TYPE_VARDICT);
  g_variant_builder_add(&options, "{sv}", "handle_token",
                        g_variant_new_string(b->token.c_str()));
  g_variant_builder_add(&options, "{sv}", "autostart",
                        g_variant_new_boolean(autostart != nullptr &&
                                              fl_value_get_bool(autostart)));
  g_variant_builder_add(&options, "{sv}", "commandline",
                        g_variant_builder_end(&command));
  if (reason != nullptr) {
    g_variant_builder_add(&options, "{sv}", "reason",
                          g_variant_new_string(fl_value_get_string(reason)));
  }
  b->options = g_variant_ref_sink(g_variant_builder_end(&options));
  g_bus_get(
      G_BUS_TYPE_SESSION, nullptr,
      [](GObject*, GAsyncResult* result, gpointer data) {
        auto* b = static_cast<Background*>(data);
        b->bus = g_bus_get_finish(result, nullptr);
        if (b->bus == nullptr) {
          finish(b, fl_value_new_null());
          release(b);
          return;
        }
        // The request's object: the sender's unique name without its colon, dots as
        // underscores, then the token. Listened to before asking.
        std::string sender = g_dbus_connection_get_unique_name(b->bus) + 1;
        for (auto& c : sender) {
          if (c == '.') c = '_';
        }
        listen(b, std::string(kPortalPath) + "/request/" + sender + "/" +
                      b->token);
        g_dbus_connection_call(
            b->bus, kPortal, kPortalPath, "org.freedesktop.portal.Background",
            "RequestBackground", g_variant_new("(s@a{sv})", "", b->options),
            G_VARIANT_TYPE("(o)"), G_DBUS_CALL_FLAGS_NONE, -1, nullptr,
            on_requested, b);
      },
      b);
}

// ----- mainWindow -----

// The channel, to tell the application how round the window's corners are as that changes.
FlMethodChannel* channel = nullptr;

// The main window's corner radius where the desktop composites windows and the window is
// free (not maximized, tiled or full screen); square otherwise, as the system's own are.
constexpr double kCorner = 12;
double corner = 0;

// The round corners: the window's background is see-through and GTK's frame (its shadow)
// follows them; the application clips what it draws to them (window.dart's windowCorner).
constexpr char kRoundCss[] =
    "window.fogata-round { background-color: transparent; }"
    "window.fogata-round decoration { border-radius: 12px; }"
    "window.fogata-round.maximized decoration, window.fogata-round.fullscreen decoration,"
    " window.fogata-round.tiled decoration, window.fogata-round.tiled-top decoration,"
    " window.fogata-round.tiled-bottom decoration, window.fogata-round.tiled-left decoration,"
    " window.fogata-round.tiled-right decoration { border-radius: 0; }";

bool see_through(GtkWidget* window) {
  GdkScreen* screen = gtk_widget_get_screen(window);
  GdkVisual* rgba = gdk_screen_get_rgba_visual(screen);
  return rgba != nullptr && gtk_widget_get_visual(window) == rgba &&
         gdk_screen_is_composited(screen);
}

// Works out the radius again and tells the application when it changed.
void update_corner(GtkWidget* window) {
  GdkWindow* gdk_window = gtk_widget_get_window(window);
  const GdkWindowState square =
      static_cast<GdkWindowState>(GDK_WINDOW_STATE_MAXIMIZED | GDK_WINDOW_STATE_FULLSCREEN |
                                  GDK_WINDOW_STATE_TILED | GDK_WINDOW_STATE_TOP_TILED |
                                  GDK_WINDOW_STATE_BOTTOM_TILED |
                                  GDK_WINDOW_STATE_LEFT_TILED |
                                  GDK_WINDOW_STATE_RIGHT_TILED);
  const bool free = gdk_window == nullptr ||
                    (gdk_window_get_state(gdk_window) & square) == 0;
  const double now = see_through(window) && free ? kCorner : 0;
  if (now == corner) return;
  corner = now;
  if (channel != nullptr) {
    fl_method_channel_invoke_method(channel, "windowCorner",
                                    fl_value_new_float(corner), nullptr, nullptr,
                                    nullptr);
  }
}

gboolean on_window_state(GtkWidget* window, GdkEventWindowState*, gpointer) {
  update_corner(window);
  return FALSE;
}

void on_composited_changed(GdkScreen*, gpointer window) {
  update_corner(GTK_WIDGET(window));
}

// The splash window (see my_application.cc) becomes the main one: its frame, the
// smallest size (960 x 640, or the work area if that is smaller: a window that may not
// be made smaller than the screen has its bottom under the panel), 1440 x 900 or the
// work area if that is smaller, centred on the monitor it is on (window.dart's
// mainWindowSizes does the same on Windows and macOS). It is hidden while it changes and
// shown again: as a new window, which is how Wayland, where no window places itself,
// lets the compositor place it. Answers the corner radius it has (0 for square corners).
void main_window(FlView* view, FlMethodCall* call) {
  GtkWidget* widget = gtk_widget_get_toplevel(GTK_WIDGET(view));
  GtkWindow* window = GTK_WINDOW(widget);
  int width = 1440;
  int height = 900;
  int min_width = 960;
  int min_height = 640;
  GdkRectangle area = {0, 0, width, height};
  GdkWindow* gdk_window = gtk_widget_get_window(widget);
  if (gdk_window != nullptr) {
    GdkMonitor* monitor = gdk_display_get_monitor_at_window(
        gtk_widget_get_display(widget), gdk_window);
    if (monitor != nullptr) {
      gdk_monitor_get_workarea(monitor, &area);
      if (area.width > 0 && area.height > 0) {
        min_width = MIN(min_width, area.width);
        min_height = MIN(min_height, area.height);
      }
      width = MAX(min_width, MIN(width, area.width));
      height = MAX(min_height, MIN(height, area.height));
    }
  }
  // Hidden meanwhile: it shows again at its new size and place.
  gtk_widget_hide(widget);
  const bool round = see_through(widget);
  if (round) {
    g_autoptr(GtkCssProvider) css = gtk_css_provider_new();
    gtk_css_provider_load_from_data(css, kRoundCss, -1, nullptr);
    gtk_style_context_add_provider_for_screen(
        gtk_widget_get_screen(widget), GTK_STYLE_PROVIDER(css),
        GTK_STYLE_PROVIDER_PRIORITY_APPLICATION);
    gtk_style_context_add_class(gtk_widget_get_style_context(widget),
                                "fogata-round");
    g_signal_connect(widget, "window-state-event", G_CALLBACK(on_window_state),
                     nullptr);
    g_signal_connect_object(gtk_widget_get_screen(widget), "composited-changed",
                            G_CALLBACK(on_composited_changed), widget,
                            static_cast<GConnectFlags>(0));
  } else {
    GdkRGBA black;
    gdk_rgba_parse(&black, "#000000");
    fl_view_set_background_color(view, &black);
    gtk_widget_set_app_paintable(widget, FALSE);
  }
  gtk_window_set_decorated(window, TRUE);
  gtk_widget_set_size_request(widget, min_width, min_height);
  gtk_window_resize(window, width, height);
  gtk_window_move(window, area.x + (area.width - width) / 2,
                  area.y + (area.height - height) / 2);
  gtk_widget_show(widget);
  gtk_window_present(window);
  corner = round ? kCorner : 0;
  update_corner(widget);
  respond(call, fl_value_new_float(corner));
}

void method_call_cb(FlMethodChannel*, FlMethodCall* call, gpointer view) {
  const gchar* method = fl_method_call_get_name(call);
  if (strcmp(method, "requestBackground") == 0) {
    request_background(call);
  } else if (strcmp(method, "mainWindow") == 0) {
    main_window(FL_VIEW(view), call);
  } else {
    g_autoptr(FlMethodResponse) response =
        FL_METHOD_RESPONSE(fl_method_not_implemented_response_new());
    fl_method_call_respond(call, response, nullptr);
  }
}

}  // namespace

void desktop_channel_register(FlView* view) {
  FlEngine* engine = fl_view_get_engine(view);
  g_autoptr(FlStandardMethodCodec) codec = fl_standard_method_codec_new();
  // Kept for the life of the application.
  channel = fl_method_channel_new(fl_engine_get_binary_messenger(engine),
                                  "fogata/desktop", FL_METHOD_CODEC(codec));
  fl_method_channel_set_method_call_handler(channel, method_call_cb, view,
                                            nullptr);
}
