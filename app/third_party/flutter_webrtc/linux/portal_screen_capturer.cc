#include "portal_screen_capturer.h"

#include "desktop_session.h"

#include <cstdlib>
#include <cstring>

#ifdef HAVE_PIPEWIRE
#include <gio/gio.h>
#include <gio/gunixfdlist.h>
#include <pipewire/pipewire.h>
#include <spa/buffer/meta.h>
#include <spa/param/buffers.h>
#include <spa/param/video/format-utils.h>
#include <spa/pod/builder.h>
#include <time.h>
#include <unistd.h>

#include <algorithm>
#include <atomic>
#include <iostream>
#include <memory>
#include <vector>

#include "portal_i420.h"
#endif

namespace flutter_webrtc_plugin {

bool PortalScreenCapturer::Available() {
#ifdef HAVE_PIPEWIRE
  return WaylandSession();
#else
  return false;
#endif
}

#ifndef HAVE_PIPEWIRE

struct PortalScreenCapturer::State {};

PortalScreenCapturer::PortalScreenCapturer() = default;
PortalScreenCapturer::~PortalScreenCapturer() = default;

void PortalScreenCapturer::Start(bool, int, FrameCallback, StartedCallback started,
                                 EndedCallback) {
  started(false, "this build cannot capture the screen through the portal");
}

void PortalScreenCapturer::SetFps(int) {}

bool PortalScreenCapturer::SharesScreen() const {
  return false;
}

void PortalScreenCapturer::Stop() {}

#else

struct PortalScreenCapturer::State {
  bool cursor = true;
  // Read on PipeWire's thread, changed from the platform thread.
  std::atomic<int> fps{30};
  FrameCallback on_frame;
  StartedCallback started;
  EndedCallback ended;
  bool stopped = false;

  GDBusConnection* bus = nullptr;
  std::string session;
  guint closed_signal = 0;
  // The portal request still waiting for its answer (one at a time), and the
  // object it answers on.
  guint request_signal = 0;
  std::string request;
  uint32_t node = 0;
  // What the dialog offered (kMonitor, kWindow) and what was chosen, as the
  // portal tells it (0 for a portal older than version 3, which does not).
  uint32_t types = 0;
  uint32_t source_type = 0;
  // For PipeWire's thread to reach the state on the GLib main thread.
  std::weak_ptr<State> self;

  pw_thread_loop* loop = nullptr;
  pw_context* context = nullptr;
  pw_core* core = nullptr;
  pw_stream* stream = nullptr;
  spa_hook stream_listener{};
  spa_video_info_raw format{};
  // When the next frame is due, on the monotonic clock.
  int64_t next_frame_ns = 0;
  std::vector<uint8_t> i420;

  ~State() {
    if (bus != nullptr) g_object_unref(bus);
  }
};

namespace {

using StatePtr = std::shared_ptr<PortalScreenCapturer::State>;

constexpr char kPortal[] = "org.freedesktop.portal.Desktop";
constexpr char kPortalPath[] = "/org/freedesktop/portal/desktop";
constexpr char kScreenCast[] = "org.freedesktop.portal.ScreenCast";
constexpr char kRequest[] = "org.freedesktop.portal.Request";
constexpr char kSession[] = "org.freedesktop.portal.Session";

constexpr uint32_t kMonitor = 1;
constexpr uint32_t kWindow = 2;
constexpr uint32_t kCursorHidden = 1;
constexpr uint32_t kCursorEmbedded = 2;

int64_t NowNs() {
  timespec t;
  clock_gettime(CLOCK_MONOTONIC, &t);
  return int64_t(t.tv_sec) * 1000000000 + t.tv_nsec;
}

// Request and session handles must be unique on the bus connection.
std::string Token() {
  static int next = 0;
  return "fogata" + std::to_string(getpid()) + "_" + std::to_string(++next);
}

// The object a portal request answers on: its path is made from the caller's
// unique bus name and the request's token.
std::string RequestPath(GDBusConnection* bus, const std::string& token) {
  std::string sender = g_dbus_connection_get_unique_name(bus) + 1;
  for (auto& c : sender) {
    if (c == '.') c = '_';
  }
  return std::string(kPortalPath) + "/request/" + sender + "/" + token;
}

// How long a property of the portal is waited for before its default is taken.
constexpr int kPropertyTimeoutMs = 5000;

// Reads the ScreenCast interface's property [name], a number, and runs [then] with it
// on the GLib main thread, or with [otherwise] when the portal cannot tell; nothing runs
// once the capture was stopped.
void ReadProperty(const StatePtr& s,
                  const char* name,
                  uint32_t otherwise,
                  std::function<void(const StatePtr&, uint32_t)> then) {
  struct Read {
    StatePtr state;
    uint32_t otherwise;
    std::function<void(const StatePtr&, uint32_t)> then;
  };
  g_dbus_connection_call(
      s->bus, kPortal, kPortalPath, "org.freedesktop.DBus.Properties", "Get",
      g_variant_new("(ss)", kScreenCast, name), G_VARIANT_TYPE("(v)"),
      G_DBUS_CALL_FLAGS_NONE, kPropertyTimeoutMs, nullptr,
      [](GObject* source, GAsyncResult* result, gpointer data) {
        std::unique_ptr<Read> read(static_cast<Read*>(data));
        GVariant* reply =
            g_dbus_connection_call_finish(G_DBUS_CONNECTION(source), result, nullptr);
        uint32_t found = read->otherwise;
        if (reply != nullptr) {
          GVariant* value = nullptr;
          g_variant_get(reply, "(v)", &value);
          if (value != nullptr && g_variant_is_of_type(value, G_VARIANT_TYPE_UINT32)) {
            found = g_variant_get_uint32(value);
          }
          if (value != nullptr) g_variant_unref(value);
          g_variant_unref(reply);
        }
        if (read->state->stopped) return;
        read->then(read->state, found);
      },
      new Read{s, otherwise, std::move(then)});
}

// Asks the portal for a session, then for what to share.
void CreateSession(const StatePtr& s, uint32_t types, uint32_t cursor_mode);

void StopState(const StatePtr& s) {
  if (s->stopped) return;
  s->stopped = true;
  if (s->loop != nullptr) {
    pw_thread_loop_lock(s->loop);
    if (s->stream != nullptr) {
      pw_stream_disconnect(s->stream);
      pw_stream_destroy(s->stream);
      s->stream = nullptr;
    }
    if (s->core != nullptr) {
      pw_core_disconnect(s->core);
      s->core = nullptr;
    }
    pw_thread_loop_unlock(s->loop);
    pw_thread_loop_stop(s->loop);
    if (s->context != nullptr) {
      pw_context_destroy(s->context);
      s->context = nullptr;
    }
    pw_thread_loop_destroy(s->loop);
    s->loop = nullptr;
  }
  if (s->bus != nullptr && s->closed_signal != 0) {
    g_dbus_connection_signal_unsubscribe(s->bus, s->closed_signal);
    s->closed_signal = 0;
  }
  // A request still open (the dialog showing) is closed with its dialog, and
  // its answer no longer listened for.
  if (s->bus != nullptr && s->request_signal != 0) {
    g_dbus_connection_signal_unsubscribe(s->bus, s->request_signal);
    s->request_signal = 0;
    g_dbus_connection_call(s->bus, kPortal, s->request.c_str(), kRequest, "Close",
                           nullptr, nullptr, G_DBUS_CALL_FLAGS_NONE, -1, nullptr,
                           nullptr, nullptr);
    s->request.clear();
  }
  if (s->bus != nullptr && !s->session.empty()) {
    g_dbus_connection_call(s->bus, kPortal, s->session.c_str(), kSession,
                           "Close", nullptr, nullptr, G_DBUS_CALL_FLAGS_NONE, -1,
                           nullptr, nullptr, nullptr);
    s->session.clear();
  }
}

void Finish(const StatePtr& s, bool started, const std::string& error) {
  if (!s->started) return;
  auto callback = std::move(s->started);
  s->started = nullptr;
  callback(started, error);
}

void Fail(const StatePtr& s, const std::string& error) {
  StopState(s);
  Finish(s, false, error);
}

// One portal call that answers through a Request object's Response signal.
struct Pending {
  StatePtr state;
  std::function<void(const StatePtr&, uint32_t, GVariant*)> then;
  guint subscription = 0;
  bool done = false;
};

// Stops listening for [pending]'s answer, unless that was done already (its
// answer came, or the capture stopped).
void Forget(const std::shared_ptr<Pending>& pending) {
  const StatePtr& s = pending->state;
  if (s->request_signal == 0 || s->request_signal != pending->subscription) return;
  g_dbus_connection_signal_unsubscribe(s->bus, s->request_signal);
  s->request_signal = 0;
  s->request.clear();
}

void OnResponse(GDBusConnection*, const gchar*, const gchar*, const gchar*,
                const gchar*, GVariant* parameters, gpointer data) {
  auto pending = *static_cast<std::shared_ptr<Pending>*>(data);
  Forget(pending);
  if (pending->done) return;
  pending->done = true;
  if (pending->state->stopped) return;
  uint32_t response = 2;
  GVariant* results = nullptr;
  g_variant_get(parameters, "(u@a{sv})", &response, &results);
  pending->then(pending->state, response, results);
  if (results != nullptr) g_variant_unref(results);
}

void OnCalled(GObject* source, GAsyncResult* result, gpointer data) {
  auto holder = static_cast<std::shared_ptr<Pending>*>(data);
  auto pending = *holder;
  delete holder;
  GError* error = nullptr;
  GVariant* reply =
      g_dbus_connection_call_finish(G_DBUS_CONNECTION(source), result, &error);
  if (reply != nullptr) {
    g_variant_unref(reply);
    return;
  }
  std::string message = error != nullptr ? error->message : "the portal failed";
  if (error != nullptr) g_error_free(error);
  Forget(pending);
  if (pending->done) return;
  pending->done = true;
  Fail(pending->state, message);
}

// Calls [method] of the ScreenCast interface with [parameters] (whose options hold
// [token] as their handle_token) and runs [then] with the request's response.
void Ask(const StatePtr& s,
         const char* method,
         GVariant* parameters,
         const std::string& token,
         std::function<void(const StatePtr&, uint32_t, GVariant*)> then) {
  auto pending = std::make_shared<Pending>();
  pending->state = s;
  pending->then = std::move(then);
  // Listening before calling: the answer may come before the call's reply.
  s->request = RequestPath(s->bus, token);
  pending->subscription = g_dbus_connection_signal_subscribe(
      s->bus, kPortal, kRequest, "Response", s->request.c_str(),
      nullptr, G_DBUS_SIGNAL_FLAGS_NO_MATCH_RULE, OnResponse,
      new std::shared_ptr<Pending>(pending),
      [](gpointer data) { delete static_cast<std::shared_ptr<Pending>*>(data); });
  s->request_signal = pending->subscription;
  g_dbus_connection_call(s->bus, kPortal, kPortalPath, kScreenCast, method,
                         parameters, G_VARIANT_TYPE("(o)"),
                         G_DBUS_CALL_FLAGS_NONE, -1, nullptr, OnCalled,
                         new std::shared_ptr<Pending>(pending));
}

void OnParamChanged(void* data, uint32_t id, const spa_pod* param) {
  auto s = static_cast<PortalScreenCapturer::State*>(data);
  if (param == nullptr || id != SPA_PARAM_Format) return;
  uint32_t media_type = 0;
  uint32_t media_subtype = 0;
  if (spa_format_parse(param, &media_type, &media_subtype) < 0 ||
      media_type != SPA_MEDIA_TYPE_video ||
      media_subtype != SPA_MEDIA_SUBTYPE_raw) {
    return;
  }
  spa_format_video_raw_parse(param, &s->format);
  // Plain memory the compositor copies into: no GPU buffers to import. Each
  // buffer carries a header, whose flags say when a frame is corrupted.
  uint8_t buffer[512];
  spa_pod_builder b = SPA_POD_BUILDER_INIT(buffer, sizeof(buffer));
  const spa_pod* params[2];
  params[0] = static_cast<const spa_pod*>(spa_pod_builder_add_object(
      &b, SPA_TYPE_OBJECT_ParamBuffers, SPA_PARAM_Buffers,
      SPA_PARAM_BUFFERS_dataType,
      SPA_POD_Int((1 << SPA_DATA_MemFd) | (1 << SPA_DATA_MemPtr))));
  params[1] = static_cast<const spa_pod*>(spa_pod_builder_add_object(
      &b, SPA_TYPE_OBJECT_ParamMeta, SPA_PARAM_Meta, SPA_PARAM_META_type,
      SPA_POD_Id(SPA_META_Header), SPA_PARAM_META_size,
      SPA_POD_Int(sizeof(spa_meta_header))));
  pw_stream_update_params(s->stream, params, 2);
}

void OnProcess(void* data) {
  auto s = static_cast<PortalScreenCapturer::State*>(data);
  pw_buffer* buffer = pw_stream_dequeue_buffer(s->stream);
  if (buffer == nullptr) return;
  spa_buffer* spa = buffer->buffer;
  spa_data* d = &spa->datas[0];
  auto header = static_cast<spa_meta_header*>(
      spa_buffer_find_meta_data(spa, SPA_META_Header, sizeof(spa_meta_header)));
  const bool corrupted =
      (header != nullptr && (header->flags & SPA_META_HEADER_FLAG_CORRUPTED) != 0) ||
      (d->chunk->flags & SPA_CHUNK_FLAG_CORRUPTED) != 0;
  const int width = int(s->format.size.width);
  const int height = int(s->format.size.height);
  const int64_t now = NowNs();
  const auto format = s->format.format;
  const bool known = format == SPA_VIDEO_FORMAT_BGRx || format == SPA_VIDEO_FORMAT_BGRA ||
                     format == SPA_VIDEO_FORMAT_RGBx || format == SPA_VIDEO_FORMAT_RGBA;
  // At most `fps` frames a second on average. Frames come at the rate asked for with
  // some jitter, so one that is a little early still counts: dropping it would leave the
  // next one late and keep about one frame in two. After a pause in frames the schedule
  // starts again from now, without a burst.
  const int64_t interval = 1000000000LL / std::max(1, s->fps.load());
  const bool due = now >= s->next_frame_ns - interval / 4;
  if (!corrupted && known && d->data != nullptr && d->chunk->size > 0 && width > 0 &&
      height > 0 && due && s->on_frame) {
    s->next_frame_ns =
        std::max(s->next_frame_ns + interval, now + interval * 3 / 4);
    const int stride = d->chunk->stride > 0 ? d->chunk->stride : width * 4;
    // The picture's rows must lie within the buffer: the offset, stride and size come
    // with each buffer, and a stride shorter than a row would read the next row's start.
    const uint64_t row = uint64_t(width) * 4;
    const uint64_t offset = d->chunk->offset;
    const uint64_t span = uint64_t(stride) * uint64_t(height - 1) + row;
    if (uint64_t(stride) < row || offset > d->maxsize || span > d->maxsize - offset) {
      pw_stream_queue_buffer(s->stream, buffer);
      return;
    }
    const int half_w = (width + 1) / 2;
    const int half_h = (height + 1) / 2;
    s->i420.resize(size_t(width) * height + 2 * size_t(half_w) * half_h);
    uint8_t* y = s->i420.data();
    uint8_t* u = y + size_t(width) * height;
    uint8_t* v = u + size_t(half_w) * half_h;
    const auto pixels = static_cast<const uint8_t*>(d->data) + d->chunk->offset;
    const bool bgr = format == SPA_VIDEO_FORMAT_BGRx || format == SPA_VIDEO_FORMAT_BGRA;
    ToI420(pixels, stride, width, height, bgr, y, u, v);
    s->on_frame(width, height, y, width, u, half_w, v, half_w);
  }
  pw_stream_queue_buffer(s->stream, buffer);
}

// The stream failed after it started (the compositor's PipeWire went away, or
// stopped the stream): the capture ends, as when the system ends it, so the
// track does not stay live with no frames. Runs on the GLib main thread.
gboolean OnStreamFailed(gpointer data) {
  std::unique_ptr<std::weak_ptr<PortalScreenCapturer::State>> holder(
      static_cast<std::weak_ptr<PortalScreenCapturer::State>*>(data));
  StatePtr s = holder->lock();
  if (s == nullptr || s->stopped) return G_SOURCE_REMOVE;
  if (s->started) {
    Fail(s, "the PipeWire stream failed");
    return G_SOURCE_REMOVE;
  }
  auto ended = s->ended;
  StopState(s);
  if (ended) ended();
  return G_SOURCE_REMOVE;
}

void OnStateChanged(void* data, pw_stream_state, pw_stream_state state,
                    const char* error) {
  if (state != PW_STREAM_STATE_ERROR) return;
  std::cerr << "[PortalScreenCapturer] stream error: " << (error ? error : "") << "\n";
  auto s = static_cast<PortalScreenCapturer::State*>(data);
  // The stream can only be stopped from outside PipeWire's thread.
  g_idle_add(OnStreamFailed, new std::weak_ptr<PortalScreenCapturer::State>(s->self));
}

bool StartStream(const StatePtr& s, int fd, uint32_t node) {
  static const bool initialized = (pw_init(nullptr, nullptr), true);
  (void)initialized;
  static pw_stream_events events = [] {
    pw_stream_events e{};
    e.version = PW_VERSION_STREAM_EVENTS;
    e.state_changed = OnStateChanged;
    e.param_changed = OnParamChanged;
    e.process = OnProcess;
    return e;
  }();
  s->loop = pw_thread_loop_new("fogata-screen", nullptr);
  if (s->loop == nullptr) return false;
  s->context = pw_context_new(pw_thread_loop_get_loop(s->loop), nullptr, 0);
  if (s->context == nullptr || pw_thread_loop_start(s->loop) < 0) {
    close(fd);
    return false;
  }
  pw_thread_loop_lock(s->loop);
  s->core = pw_context_connect_fd(s->context, fd, nullptr, 0);
  if (s->core == nullptr) {
    pw_thread_loop_unlock(s->loop);
    return false;
  }
  s->stream = pw_stream_new(
      s->core, "Fogata screen",
      pw_properties_new(PW_KEY_MEDIA_TYPE, "Video", PW_KEY_MEDIA_CATEGORY,
                        "Capture", PW_KEY_MEDIA_ROLE, "Screen", nullptr));
  pw_stream_add_listener(s->stream, &s->stream_listener, &events, s.get());
  uint8_t buffer[1024];
  spa_pod_builder b = SPA_POD_BUILDER_INIT(buffer, sizeof(buffer));
  spa_rectangle size = SPA_RECTANGLE(1920, 1080);
  spa_rectangle min_size = SPA_RECTANGLE(1, 1);
  spa_rectangle max_size = SPA_RECTANGLE(16384, 16384);
  spa_fraction rate = SPA_FRACTION(uint32_t(s->fps.load()), 1);
  spa_fraction min_rate = SPA_FRACTION(0, 1);
  spa_fraction max_rate = SPA_FRACTION(120, 1);
  const spa_pod* params[1];
  params[0] = static_cast<const spa_pod*>(spa_pod_builder_add_object(
      &b, SPA_TYPE_OBJECT_Format, SPA_PARAM_EnumFormat, SPA_FORMAT_mediaType,
      SPA_POD_Id(SPA_MEDIA_TYPE_video), SPA_FORMAT_mediaSubtype,
      SPA_POD_Id(SPA_MEDIA_SUBTYPE_raw), SPA_FORMAT_VIDEO_format,
      SPA_POD_CHOICE_ENUM_Id(5, SPA_VIDEO_FORMAT_BGRx, SPA_VIDEO_FORMAT_BGRx,
                             SPA_VIDEO_FORMAT_BGRA, SPA_VIDEO_FORMAT_RGBx,
                             SPA_VIDEO_FORMAT_RGBA),
      SPA_FORMAT_VIDEO_size,
      SPA_POD_CHOICE_RANGE_Rectangle(&size, &min_size, &max_size),
      SPA_FORMAT_VIDEO_framerate,
      SPA_POD_CHOICE_RANGE_Fraction(&rate, &min_rate, &max_rate)));
  const int connected = pw_stream_connect(
      s->stream, PW_DIRECTION_INPUT, node,
      static_cast<pw_stream_flags>(PW_STREAM_FLAG_AUTOCONNECT |
                                   PW_STREAM_FLAG_MAP_BUFFERS),
      params, 1);
  pw_thread_loop_unlock(s->loop);
  return connected >= 0;
}

void OnRemote(GObject* source, GAsyncResult* result, gpointer data) {
  auto holder = static_cast<StatePtr*>(data);
  StatePtr s = *holder;
  delete holder;
  GUnixFDList* fds = nullptr;
  GError* error = nullptr;
  GVariant* reply = g_dbus_connection_call_with_unix_fd_list_finish(
      G_DBUS_CONNECTION(source), &fds, result, &error);
  if (s->stopped) {
    if (reply != nullptr) g_variant_unref(reply);
    if (fds != nullptr) g_object_unref(fds);
    if (error != nullptr) g_error_free(error);
    return;
  }
  if (reply == nullptr) {
    std::string message = error != nullptr ? error->message : "no PipeWire";
    if (error != nullptr) g_error_free(error);
    Fail(s, message);
    return;
  }
  gint32 index = 0;
  g_variant_get(reply, "(h)", &index);
  g_variant_unref(reply);
  const int fd = fds != nullptr ? g_unix_fd_list_get(fds, index, nullptr) : -1;
  if (fds != nullptr) g_object_unref(fds);
  if (fd < 0) {
    Fail(s, "the portal gave no PipeWire connection");
    return;
  }
  if (!StartStream(s, fd, s->node)) {
    Fail(s, "the PipeWire stream did not start");
    return;
  }
  Finish(s, true, "");
}

// The system ended the session: its own "stop sharing", or the shared window
// closed.
void OnClosed(GDBusConnection*, const gchar*, const gchar*, const gchar*,
              const gchar*, GVariant*, gpointer data) {
  StatePtr s = *static_cast<StatePtr*>(data);
  if (s->stopped) return;
  s->session.clear();
  auto ended = s->ended;
  StopState(s);
  if (ended) ended();
}

std::string SessionHandle(GVariant* results) {
  GVariant* value = g_variant_lookup_value(results, "session_handle", nullptr);
  if (value == nullptr) return "";
  std::string handle;
  if (g_variant_is_of_type(value, G_VARIANT_TYPE_STRING) ||
      g_variant_is_of_type(value, G_VARIANT_TYPE_OBJECT_PATH)) {
    handle = g_variant_get_string(value, nullptr);
  }
  g_variant_unref(value);
  return handle;
}

void StartCast(const StatePtr& s) {
  const std::string token = Token();
  GVariantBuilder options;
  g_variant_builder_init(&options, G_VARIANT_TYPE_VARDICT);
  g_variant_builder_add(&options, "{sv}", "handle_token",
                        g_variant_new_string(token.c_str()));
  Ask(s, "Start", g_variant_new("(osa{sv})", s->session.c_str(), "", &options),
      token, [](const StatePtr& s, uint32_t response, GVariant* results) {
        if (response == 1) {
          Fail(s, "cancelled");
          return;
        }
        GVariant* streams =
            response == 0 ? g_variant_lookup_value(results, "streams",
                                                   G_VARIANT_TYPE("a(ua{sv})"))
                          : nullptr;
        if (streams == nullptr || g_variant_n_children(streams) == 0) {
          if (streams != nullptr) g_variant_unref(streams);
          Fail(s, "the screen cast did not start");
          return;
        }
        GVariant* properties = nullptr;
        g_variant_get_child(streams, 0, "(u@a{sv})", &s->node, &properties);
        if (properties != nullptr) {
          uint32_t source_type = 0;
          if (g_variant_lookup(properties, "source_type", "u", &source_type)) {
            s->source_type = source_type;
          }
          g_variant_unref(properties);
        }
        g_variant_unref(streams);
        GVariantBuilder none;
        g_variant_builder_init(&none, G_VARIANT_TYPE_VARDICT);
        g_dbus_connection_call_with_unix_fd_list(
            s->bus, kPortal, kPortalPath, kScreenCast, "OpenPipeWireRemote",
            g_variant_new("(oa{sv})", s->session.c_str(), &none),
            G_VARIANT_TYPE("(h)"), G_DBUS_CALL_FLAGS_NONE, -1, nullptr, nullptr,
            OnRemote, new StatePtr(s));
      });
}

void SelectSources(const StatePtr& s, uint32_t types, uint32_t cursor_mode) {
  const std::string token = Token();
  GVariantBuilder options;
  g_variant_builder_init(&options, G_VARIANT_TYPE_VARDICT);
  g_variant_builder_add(&options, "{sv}", "handle_token",
                        g_variant_new_string(token.c_str()));
  g_variant_builder_add(&options, "{sv}", "types", g_variant_new_uint32(types));
  g_variant_builder_add(&options, "{sv}", "multiple", g_variant_new_boolean(FALSE));
  g_variant_builder_add(&options, "{sv}", "cursor_mode",
                        g_variant_new_uint32(cursor_mode));
  Ask(s, "SelectSources",
      g_variant_new("(oa{sv})", s->session.c_str(), &options), token,
      [](const StatePtr& s, uint32_t response, GVariant*) {
        if (response != 0) {
          Fail(s, response == 1 ? "cancelled" : "no source was chosen");
          return;
        }
        StartCast(s);
      });
}

}  // namespace

PortalScreenCapturer::PortalScreenCapturer()
    : state_(std::make_shared<State>()) {
  state_->self = state_;
}

PortalScreenCapturer::~PortalScreenCapturer() {
  Stop();
}

void PortalScreenCapturer::Start(bool cursor,
                                 int fps,
                                 FrameCallback on_frame,
                                 StartedCallback started,
                                 EndedCallback ended) {
  StatePtr s = state_;
  s->cursor = cursor;
  s->fps = fps > 0 ? fps : 30;
  s->on_frame = std::move(on_frame);
  s->started = std::move(started);
  s->ended = std::move(ended);
  // Nothing here waits on the bus or the portal: a portal that hangs must not freeze
  // the window.
  g_bus_get(
      G_BUS_TYPE_SESSION, nullptr,
      [](GObject*, GAsyncResult* result, gpointer data) {
        std::unique_ptr<StatePtr> holder(static_cast<StatePtr*>(data));
        const StatePtr s = *holder;
        GError* error = nullptr;
        GDBusConnection* bus = g_bus_get_finish(result, &error);
        if (s->stopped) {
          if (bus != nullptr) g_object_unref(bus);
          if (error != nullptr) g_error_free(error);
          return;
        }
        if (bus == nullptr) {
          std::string message = error != nullptr ? error->message : "no session bus";
          if (error != nullptr) g_error_free(error);
          Fail(s, message);
          return;
        }
        s->bus = bus;
        // Screens and windows in one dialog, where the system offers both.
        ReadProperty(s, "AvailableSourceTypes", kMonitor,
                     [](const StatePtr& s, uint32_t sources) {
          ReadProperty(s, "AvailableCursorModes", kCursorHidden,
                       [sources](const StatePtr& s, uint32_t cursors) {
            uint32_t types = sources & (kMonitor | kWindow);
            if (types == 0) types = kMonitor;
            const uint32_t cursor_mode = s->cursor && (cursors & kCursorEmbedded) != 0
                                             ? kCursorEmbedded
                                             : kCursorHidden;
            CreateSession(s, types, cursor_mode);
          });
        });
      },
      new StatePtr(s));
}

void PortalScreenCapturer::SetFps(int fps) {
  state_->fps = fps > 0 ? fps : 30;
}

bool PortalScreenCapturer::SharesScreen() const {
  const State& s = *state_;
  return s.source_type == kMonitor || (s.source_type == 0 && s.types == kMonitor);
}

void PortalScreenCapturer::Stop() {
  StopState(state_);
}

namespace {

void CreateSession(const StatePtr& s, uint32_t types, uint32_t cursor_mode) {
  const std::string token = Token();
  const std::string session_token = Token();
  GVariantBuilder options;
  g_variant_builder_init(&options, G_VARIANT_TYPE_VARDICT);
  g_variant_builder_add(&options, "{sv}", "handle_token",
                        g_variant_new_string(token.c_str()));
  g_variant_builder_add(&options, "{sv}", "session_handle_token",
                        g_variant_new_string(session_token.c_str()));
  // The session's handle is made like a request's: known before the answer,
  // so a capture stopped while the session is being made still closes it.
  std::string predicted = RequestPath(s->bus, session_token);
  predicted.replace(predicted.find("/request/"), 9, "/session/");
  s->session = predicted;
  Ask(s, "CreateSession", g_variant_new("(a{sv})", &options), token,
      [types, cursor_mode](const StatePtr& s, uint32_t response, GVariant* results) {
        const std::string handle = response == 0 ? SessionHandle(results) : "";
        if (handle.empty()) {
          Fail(s, "the portal made no screen cast session");
          return;
        }
        s->session = handle;
        s->types = types;
        s->closed_signal = g_dbus_connection_signal_subscribe(
            s->bus, kPortal, kSession, "Closed", handle.c_str(), nullptr,
            G_DBUS_SIGNAL_FLAGS_NO_MATCH_RULE, OnClosed, new StatePtr(s),
            [](gpointer data) { delete static_cast<StatePtr*>(data); });
        SelectSources(s, types, cursor_mode);
      });
}

}  // namespace

#endif

}  // namespace flutter_webrtc_plugin
