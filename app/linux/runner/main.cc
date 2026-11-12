#include <dlfcn.h>
#include <malloc.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#include <string>

#include "my_application.h"

// A device name that can go between quotes in PipeWire's properties as it is.
static bool PlainName(const char* name) {
  if (name == nullptr || name[0] == '\0') return false;
  for (const char* c = name; *c != '\0'; c++) {
    if (!g_ascii_isalnum(*c) && strchr("._-:", *c) == nullptr) return false;
  }
  return true;
}

// The core reads the X server's idle time (MIT-SCREEN-SAVER) through a connection of its own,
// from a thread besides GTK's. Xlib allows that only after XInitThreads, and only when it
// is the first Xlib call of the process: some builds of libX11 call it when they load, others
// do not. GTK does not call it either. It is looked up in the library GTK has already loaded,
// so the runner needs no link to it, and it does nothing when threads are already set up.
static void InitXlibThreads() {
  void* init = dlsym(RTLD_DEFAULT, "XInitThreads");
  if (init != nullptr) reinterpret_cast<int (*)()>(init)();
}

// Whether NVIDIA's EGL library is in the process: Flutter draws through it on an NVIDIA
// card. When the engine shuts down, its compositor makes its own context current on this
// thread to delete its shaders, and then the engine destroys its contexts and terminates
// the display with that context still current. Some NVIDIA drivers keep the context
// and destroys it in its own handler for the end of the process, which exit runs after
// main returns, and which reads through a null pointer there: quitting ended with
// SIGSEGV under KDE Plasma Wayland, and the desktop reported a crash. Releasing that
// context before the display is terminated only moves the same fault into the engine's
// own eglDestroyContext, and releasing it afterwards (eglReleaseThread) faults the same
// way: the engine's contexts cannot be destroyed cleanly from here. So with that library
// loaded the process ends without the exit handlers once the application and its engine
// are gone. Nothing of Fogata's waits for them: the core keeps no state to write at exit
// (its files are written as it goes, and its thread-local values are counters
// of tests), the plugins hold nothing a closed process does not give back, and the
// standard streams are flushed first.
static bool NvidiaEgl() {
  void* nvidia = dlopen("libEGL_nvidia.so.0", RTLD_LAZY | RTLD_NOLOAD);
  if (nvidia == nullptr) return false;
  dlclose(nvidia);
  return true;
}

// glibc gives a thread that finds the allocator busy an arena of its own, up to eight for
// each core, and memory freed in one arena is of no use to another. A call's dozens of
// threads (WebRTC's, the core's, the engine's) spread what they hold over many, each kept
// with holes: two copies in a voice call of an hour held about 80 MB less each with two
// arenas (some 520 MB against 600) and grew about half as fast, and a screen shared to
// five kept its 30 frames a second and its processor time. Set before any thread starts;
// MALLOC_ARENA_MAX in the environment, glibc's own setting for this, is kept.
static void FewArenas() {
#ifdef __GLIBC__
  if (getenv("MALLOC_ARENA_MAX") == nullptr) mallopt(M_ARENA_MAX, 2);
#endif
}

int main(int argc, char** argv) {
  FewArenas();
  InitXlibThreads();
  // Fogata's own sounds (someone coming in, a message, a ringing call) reach PipeWire
  // through ALSA, which names their stream after the executable: "PipeWire ALSA
  // [fogata]", shared with any other program of that name, and the session manager keeps
  // its volume and mute under that name. Named as the application, they show as Fogata
  // in the system's volume controls, with a volume of their own. When PULSE_SINK sends
  // the rest of the application's sound (calls, videos) to one device, the sounds go
  // there too, as test copies that use a null sink expect. Set before any thread starts;
  // a value already in the environment is kept.
  std::string props =
      "{ application.name = \"Fogata\" application.id = \"dev.holypanda.fogata\" "
      "application.icon-name = \"dev.holypanda.fogata\" "
      "media.name = \"Fogata sounds\" node.description = \"Fogata sounds\"";
  const char* sink = getenv("PULSE_SINK");
  if (PlainName(sink)) props += std::string(" target.object = \"") + sink + "\"";
  props += " }";
  setenv("PIPEWIRE_ALSA", props.c_str(), 0);
  MyApplication* app = my_application_new();
  const int status = g_application_run(G_APPLICATION(app), argc, argv);
  // The engine shuts down as the application goes (my_application_dispose).
  g_object_unref(app);
  if (NvidiaEgl()) {
    fflush(nullptr);
    _exit(status);
  }
  return status;
}
