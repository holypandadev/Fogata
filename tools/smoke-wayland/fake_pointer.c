// Moves the pointer and presses its left button in KWin, as a person's hand would, through
// KWin's fake input (org_kde_kwin_fake_input, which KWin gives any client when started with
// KWIN_WAYLAND_NO_PERMISSION_CHECKS=1, as session.sh can). The steps are its arguments, in
// the compositor's coordinates:
//   to <x> <y>                          the pointer there at once
//   glide <x0> <y0> <x1> <y1> <n> <ms>  n moves from one point to the other, ms between
//   press | release                     the left button
//   wait <ms>
// tools/smoke-wayland/drop.sh drags with it.
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <wayland-client.h>

#include "fake-input-client-protocol.h"

#define LEFT_BUTTON 0x110

static struct org_kde_kwin_fake_input *fake;
static uint32_t fake_version;

static void global(void *data, struct wl_registry *registry, uint32_t name,
                   const char *interface, uint32_t version) {
  if (strcmp(interface, org_kde_kwin_fake_input_interface.name) == 0) {
    fake_version = version < 4 ? version : 4;
    fake = wl_registry_bind(registry, name, &org_kde_kwin_fake_input_interface, fake_version);
  }
}

static void global_remove(void *data, struct wl_registry *registry, uint32_t name) {}

static const struct wl_registry_listener listener = {global, global_remove};

static void pause_ms(long ms) {
  struct timespec t = {ms / 1000, (ms % 1000) * 1000000L};
  nanosleep(&t, NULL);
}

int main(int argc, char **argv) {
  struct wl_display *display = wl_display_connect(NULL);
  if (display == NULL) {
    fprintf(stderr, "fake pointer: no Wayland display\n");
    return 3;
  }
  struct wl_registry *registry = wl_display_get_registry(display);
  wl_registry_add_listener(registry, &listener, NULL);
  wl_display_roundtrip(display);
  if (fake == NULL || fake_version < 3) {
    fprintf(stderr, "fake pointer: the compositor offers no fake input (version 3 or later)\n");
    return 3;
  }
  org_kde_kwin_fake_input_authenticate(fake, "Fogata drop lab", "drags files");
  wl_display_roundtrip(display);
  for (int i = 1; i < argc; i++) {
    const char *step = argv[i];
    if (strcmp(step, "to") == 0 && i + 2 < argc) {
      org_kde_kwin_fake_input_pointer_motion_absolute(fake, wl_fixed_from_int(atoi(argv[i + 1])),
                                                      wl_fixed_from_int(atoi(argv[i + 2])));
      i += 2;
    } else if (strcmp(step, "glide") == 0 && i + 6 < argc) {
      int x0 = atoi(argv[i + 1]), y0 = atoi(argv[i + 2]);
      int x1 = atoi(argv[i + 3]), y1 = atoi(argv[i + 4]);
      int n = atoi(argv[i + 5]);
      long ms = atol(argv[i + 6]);
      for (int k = 1; k <= n; k++) {
        org_kde_kwin_fake_input_pointer_motion_absolute(
            fake, wl_fixed_from_double(x0 + (double)(x1 - x0) * k / n),
            wl_fixed_from_double(y0 + (double)(y1 - y0) * k / n));
        wl_display_roundtrip(display);
        pause_ms(ms);
      }
      i += 6;
    } else if (strcmp(step, "press") == 0) {
      org_kde_kwin_fake_input_button(fake, LEFT_BUTTON, 1);
    } else if (strcmp(step, "release") == 0) {
      org_kde_kwin_fake_input_button(fake, LEFT_BUTTON, 0);
    } else if (strcmp(step, "wait") == 0 && i + 1 < argc) {
      wl_display_roundtrip(display);
      pause_ms(atol(argv[i + 1]));
      i += 1;
    } else {
      fprintf(stderr, "fake pointer: what is %s?\n", step);
      return 2;
    }
    wl_display_roundtrip(display);
  }
  wl_display_disconnect(display);
  return 0;
}
