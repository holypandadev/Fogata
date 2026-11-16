// A window that drags the files named on its command line, offering what a GTK file manager
// offers: the list of files (`text/uri-list`) and, where the FileTransfer portal runs, the
// portal's targets, through which GTK hands the files to a sandboxed application by itself.
// With --uri-only it offers the list alone, as a source that knows nothing of the portal;
// with --raw its arguments are the list's entries as they are (any URI, as a web page can
// write them); with --second its window is another one (drop.sh's two drags in a row);
// with --portal-only it offers the portal's targets alone, no list; with --slow it gives its
// list three seconds after it is asked (a second drag can come meanwhile).
// It says on stderr which targets each drag offered, which one the other side asked for, and
// how the drag ended. tools/smoke-wayland/drop.sh drags from it.
//
//     drag_source [--uri-only | --portal-only] [--raw] [--second] [--slow] <file or URI>...
#include <gtk/gtk.h>
#include <stdio.h>
#include <string.h>

static gchar **uris;
static gboolean slow = FALSE;

static void on_begin(GtkWidget *widget, GdkDragContext *context, gpointer data) {
  GString *offered = g_string_new(NULL);
  for (GList *t = gdk_drag_context_list_targets(context); t != NULL; t = t->next) {
    gchar *name = gdk_atom_name(GDK_POINTER_TO_ATOM(t->data));
    g_string_append_printf(offered, " %s", name);
    g_free(name);
  }
  fprintf(stderr, "drag source: drag begun, offering%s\n", offered->str);
  g_string_free(offered, TRUE);
}

static void on_get(GtkWidget *widget, GdkDragContext *context, GtkSelectionData *selection,
                   guint info, guint time, gpointer data) {
  gchar *name = gdk_atom_name(gtk_selection_data_get_target(selection));
  if (slow && strcmp(name, "text/uri-list") == 0) {
    fprintf(stderr, "drag source: asked for %s, waiting\n", name);
    g_usleep(3 * G_USEC_PER_SEC);
  }
  gboolean given = gtk_selection_data_set_uris(selection, uris);
  fprintf(stderr, "drag source: asked for %s: %s\n", name, given ? "given" : "NOT given");
  g_free(name);
}

static gboolean on_failed(GtkWidget *widget, GdkDragContext *context, GtkDragResult result,
                          gpointer data) {
  fprintf(stderr, "drag source: drag failed (%d)\n", result);
  return FALSE;
}

static void on_end(GtkWidget *widget, GdkDragContext *context, gpointer data) {
  fprintf(stderr, "drag source: drag ended\n");
}

static void on_map(GtkWidget *widget, gpointer data) {
  fprintf(stderr, "drag source: shown\n");
}

int main(int argc, char **argv) {
  gtk_init(&argc, &argv);
  gboolean uri_only = FALSE, portal_only = FALSE, raw = FALSE, second = FALSE;
  GPtrArray *list = g_ptr_array_new();
  for (int i = 1; i < argc; i++) {
    if (strcmp(argv[i], "--uri-only") == 0) {
      uri_only = TRUE;
      continue;
    }
    if (strcmp(argv[i], "--raw") == 0) {
      raw = TRUE;
      continue;
    }
    if (strcmp(argv[i], "--portal-only") == 0) {
      portal_only = TRUE;
      continue;
    }
    if (strcmp(argv[i], "--slow") == 0) {
      slow = TRUE;
      continue;
    }
    if (strcmp(argv[i], "--second") == 0) {
      second = TRUE;
      continue;
    }
    gchar *uri = raw ? g_strdup(argv[i]) : g_filename_to_uri(argv[i], NULL, NULL);
    if (uri == NULL) {
      fprintf(stderr, "drag source: %s is no absolute path\n", argv[i]);
      return 2;
    }
    g_ptr_array_add(list, uri);
  }
  if (list->len == 0) {
    fprintf(stderr, "usage: drag_source [--uri-only | --portal-only] [--raw] [--second] "
                    "[--slow] <file>...\n");
    return 2;
  }
  g_ptr_array_add(list, NULL);
  uris = (gchar **)g_ptr_array_free(list, FALSE);

  GtkWidget *window = gtk_window_new(GTK_WINDOW_TOPLEVEL);
  gtk_window_set_title(GTK_WINDOW(window),
                       second ? "Fogata drag source two" : "Fogata drag source");
  gtk_window_set_default_size(GTK_WINDOW(window), 300, 200);
  g_signal_connect(window, "destroy", G_CALLBACK(gtk_main_quit), NULL);
  g_signal_connect(window, "map", G_CALLBACK(on_map), NULL);
  GtkWidget *box = gtk_event_box_new();
  gtk_container_add(GTK_CONTAINER(box), gtk_label_new("drag me"));
  gtk_container_add(GTK_CONTAINER(window), box);

  if (uri_only) {
    GtkTargetEntry list_only[] = {{(gchar *)"text/uri-list", 0, 0}};
    gtk_drag_source_set(box, GDK_BUTTON1_MASK, list_only, 1, GDK_ACTION_COPY);
  } else if (portal_only) {
    GtkTargetEntry portal[] = {{(gchar *)"application/vnd.portal.filetransfer", 0, 0},
                               {(gchar *)"application/vnd.portal.files", 0, 0}};
    gtk_drag_source_set(box, GDK_BUTTON1_MASK, portal, 2, GDK_ACTION_COPY);
  } else {
    gtk_drag_source_set(box, GDK_BUTTON1_MASK, NULL, 0, GDK_ACTION_COPY);
    gtk_drag_source_add_uri_targets(box);
  }
  g_signal_connect(box, "drag-begin", G_CALLBACK(on_begin), NULL);
  g_signal_connect(box, "drag-data-get", G_CALLBACK(on_get), NULL);
  g_signal_connect(box, "drag-failed", G_CALLBACK(on_failed), NULL);
  g_signal_connect(box, "drag-end", G_CALLBACK(on_end), NULL);
  gtk_widget_show_all(window);
  gtk_main();
  return 0;
}
