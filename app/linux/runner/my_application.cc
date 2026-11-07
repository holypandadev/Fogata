#include "my_application.h"

#include <flutter_linux/flutter_linux.h>
#ifdef GDK_WINDOWING_X11
#include <gdk/gdkx.h>
#endif

#include "desktop_channel.h"
#include "flutter/generated_plugin_registrant.h"

struct _MyApplication {
  GtkApplication parent_instance;
  char** dart_entrypoint_arguments;
  // The engine, from the window's destruction until the main loop has ended.
  FlEngine* engine;
};

G_DEFINE_TYPE(MyApplication, my_application, GTK_TYPE_APPLICATION)

// Called when first Flutter frame received.
static void first_frame_cb(MyApplication* self, FlView* view) {
  gtk_widget_show(gtk_widget_get_toplevel(GTK_WIDGET(view)));
}

// The view's widgets, kept from the window's destruction until the engine has shut down.
struct KeptWidgets {
  GPtrArray* widgets;
  GThread* gtk_thread;
};

// Adds a reference to `widget` and to every widget inside it to the array `data`.
static void keep_widgets(GtkWidget* widget, gpointer data) {
  g_ptr_array_add(static_cast<GPtrArray*>(data), g_object_ref(widget));
  if (GTK_IS_CONTAINER(widget)) {
    gtk_container_forall(GTK_CONTAINER(widget), keep_widgets, data);
  }
}

// Called once the engine has shut down, its threads stopped.
static void engine_gone_cb(gpointer data, GObject* engine) {
  auto* kept = static_cast<KeptWidgets*>(data);
  // Widgets only go on GTK's thread; left to the end of the process otherwise.
  if (g_thread_self() == kept->gtk_thread) {
    g_ptr_array_unref(kept->widgets);
  }
  delete kept;
}

// Called as the window is destroyed (when the application quits), before the widgets
// inside it are.
//
// Destroying the window frees the view's frame compositor while the engine still runs,
// so a frame the engine's raster thread is presenting at that moment holds the
// compositor's lock (GLib aborts the process on clearing a locked mutex) or finishes in
// freed memory. The view's widgets, which own the compositor, are kept until the engine
// has shut down, which waits for that frame.
//
// The engine is kept too, so that it shuts down once the main loop has ended, not while
// it runs: the engine's platform task runner can leave a timeout behind (one posted from
// another thread while an earlier one was running), which would run on the freed task
// runner at the loop's next turn.
//
// The view is found in the window rather than passed as the handler's data: the window
// plugin disconnects the window's first handler whose data is the view.
static void window_destroy_cb(GtkWidget* window, MyApplication* self) {
  GtkWidget* view = gtk_bin_get_child(GTK_BIN(window));
  if (!FL_IS_VIEW(view)) return;
  FlEngine* engine = fl_view_get_engine(FL_VIEW(view));
  auto* kept = new KeptWidgets{g_ptr_array_new_with_free_func(g_object_unref),
                               g_thread_self()};
  keep_widgets(view, kept->widgets);
  g_object_weak_ref(G_OBJECT(engine), engine_gone_cb, kept);
  g_set_object(&self->engine, engine);
}

// Implements GApplication::activate.
static void my_application_activate(GApplication* application) {
  MyApplication* self = MY_APPLICATION(application);
  GtkWindow* window =
      GTK_WINDOW(gtk_application_window_new(GTK_APPLICATION(application)));

  // Fogata draws its own title bar. A hidden header bar keeps GTK's client-side frame
  // (shadow and resize edges) without a system title bar.
  GtkHeaderBar* header_bar = GTK_HEADER_BAR(gtk_header_bar_new());
  gtk_widget_set_no_show_all(GTK_WIDGET(header_bar), TRUE);
  gtk_window_set_titlebar(window, GTK_WIDGET(header_bar));
  gtk_window_set_title(window, "Fogata");

  // Fogata opens as a small splash window with no frame (lib/src/splash.dart), centred;
  // the application makes it the main window through "fogata/desktop" (mainWindow in
  // desktop_channel.cc). Its corners are round, see-through where the desktop composites
  // windows.
  // (Not made unresizable: GTK then sizes a window by what its contents ask, which for
  // the view is nothing, and ignores the default size.)
  gtk_window_set_decorated(window, FALSE);
  gtk_window_set_default_size(window, 300, 360);
  gtk_widget_set_size_request(GTK_WIDGET(window), 300, 360);
  gtk_window_set_position(window, GTK_WIN_POS_CENTER);
  GdkScreen* screen = gtk_window_get_screen(window);
  GdkVisual* rgba = gdk_screen_get_rgba_visual(screen);
  const bool see_through = rgba != nullptr && gdk_screen_is_composited(screen);
  if (see_through) {
    gtk_widget_set_visual(GTK_WIDGET(window), rgba);
    gtk_widget_set_app_paintable(GTK_WIDGET(window), TRUE);
  }

  // The window icon sits in the bundle's data folder, next to the executable. Wayland
  // desktops take the icon from the .desktop file instead.
  g_autofree gchar* executable = g_file_read_link("/proc/self/exe", nullptr);
  if (executable != nullptr) {
    g_autofree gchar* folder = g_path_get_dirname(executable);
    g_autofree gchar* icon =
        g_build_filename(folder, "data", "app_icon.png", nullptr);
    gtk_window_set_icon_from_file(window, icon, nullptr);
  }

  g_autoptr(FlDartProject) project = fl_dart_project_new();
  fl_dart_project_set_dart_entrypoint_arguments(
      project, self->dart_entrypoint_arguments);

  FlView* view = fl_view_new(project);
  GdkRGBA background_color;
  gdk_rgba_parse(&background_color, see_through ? "#00000000" : "#000000");
  fl_view_set_background_color(view, &background_color);
  gtk_widget_show(GTK_WIDGET(view));
  gtk_container_add(GTK_CONTAINER(window), GTK_WIDGET(view));
  g_signal_connect(window, "destroy", G_CALLBACK(window_destroy_cb), self);

  // Show the window when Flutter renders.
  // Requires the view to be realized so we can start rendering.
  g_signal_connect_swapped(view, "first-frame", G_CALLBACK(first_frame_cb),
                           self);
  gtk_widget_realize(GTK_WIDGET(view));

  fl_register_plugins(FL_PLUGIN_REGISTRY(view));
  desktop_channel_register(view);

  gtk_widget_grab_focus(GTK_WIDGET(view));
}

// Implements GApplication::local_command_line.
static gboolean my_application_local_command_line(GApplication* application,
                                                  gchar*** arguments,
                                                  int* exit_status) {
  MyApplication* self = MY_APPLICATION(application);
  // Strip out the first argument as it is the binary name.
  self->dart_entrypoint_arguments = g_strdupv(*arguments + 1);

  g_autoptr(GError) error = nullptr;
  if (!g_application_register(application, nullptr, &error)) {
    g_warning("Failed to register: %s", error->message);
    *exit_status = 1;
    return TRUE;
  }

  g_application_activate(application);
  *exit_status = 0;

  return TRUE;
}

// Implements GApplication::startup.
static void my_application_startup(GApplication* application) {
  // MyApplication* self = MY_APPLICATION(object);

  // Perform any actions required at application startup.

  G_APPLICATION_CLASS(my_application_parent_class)->startup(application);
}

// Implements GApplication::shutdown.
static void my_application_shutdown(GApplication* application) {
  // MyApplication* self = MY_APPLICATION(object);

  // Perform any actions required at application shutdown.

  G_APPLICATION_CLASS(my_application_parent_class)->shutdown(application);
}

// Implements GObject::dispose.
static void my_application_dispose(GObject* object) {
  MyApplication* self = MY_APPLICATION(object);
  g_clear_pointer(&self->dart_entrypoint_arguments, g_strfreev);
  // After the main loop: the engine shuts down here, unless something else still holds it.
  g_clear_object(&self->engine);
  G_OBJECT_CLASS(my_application_parent_class)->dispose(object);
}

static void my_application_class_init(MyApplicationClass* klass) {
  G_APPLICATION_CLASS(klass)->activate = my_application_activate;
  G_APPLICATION_CLASS(klass)->local_command_line =
      my_application_local_command_line;
  G_APPLICATION_CLASS(klass)->startup = my_application_startup;
  G_APPLICATION_CLASS(klass)->shutdown = my_application_shutdown;
  G_OBJECT_CLASS(klass)->dispose = my_application_dispose;
}

static void my_application_init(MyApplication* self) {}

MyApplication* my_application_new() {
  // Set the program name to the application ID, which helps various systems
  // like GTK and desktop environments map this running application to its
  // corresponding .desktop file. This ensures better integration by allowing
  // the application to be recognized beyond its binary name.
  g_set_prgname(APPLICATION_ID);

  return MY_APPLICATION(g_object_new(my_application_get_type(),
                                     "application-id", APPLICATION_ID, "flags",
                                     G_APPLICATION_NON_UNIQUE, nullptr));
}
