#include "silent_input.h"

#ifdef HAVE_PIPEWIRE

#include <pipewire/pipewire.h>
#include <unistd.h>

#include <atomic>
#include <iostream>

#include <mutex>
#include <string>

namespace flutter_webrtc_plugin {

namespace {

// The input is a node PipeWire itself runs: a null sink shown as a source,
// which gives silence while nothing feeds it. It belongs to this connection,
// so it goes away with it, even if the app ends without stopping it.
struct Silence {
  pw_thread_loop* loop = nullptr;
  pw_context* context = nullptr;
  pw_core* core = nullptr;
  pw_proxy* node = nullptr;
  spa_hook node_listener = {};
  bool listening = false;
  spa_hook core_listener = {};
  bool watching = false;
  // The connection to the server was lost (PipeWire restarted, as after an update):
  // the node went with it. Written on the loop's thread.
  std::atomic<bool> lost{false};
  // The server's answer to making the node, written on the loop's thread.
  int pending = -1;
  bool answered = false;
  bool failed = false;
};

// How long the server may take to make the node.
constexpr int64_t kAnswerSeconds = 2;

void OnBound(void* data, uint32_t) {
  auto s = static_cast<Silence*>(data);
  s->answered = true;
  pw_thread_loop_signal(s->loop, false);
}

void OnError(void* data, int, int res, const char* message) {
  auto s = static_cast<Silence*>(data);
  std::cerr << "[SilentInput] " << (message ? message : "") << " (" << res
            << ")\n";
  s->failed = true;
  s->answered = true;
  pw_thread_loop_signal(s->loop, false);
}

// The server has handled every request sent before the sync, the node's
// making included; any error for it came first.
void OnDone(void* data, int seq) {
  auto s = static_cast<Silence*>(data);
  if (seq != s->pending) return;
  s->answered = true;
  pw_thread_loop_signal(s->loop, false);
}

// An error of the connection itself (id PW_ID_CORE), such as EPIPE when the server
// goes away: the input is gone, and the next Start makes it again.
void OnCoreError(void* data, uint32_t id, int, int res, const char* message) {
  if (id != PW_ID_CORE) return;
  auto s = static_cast<Silence*>(data);
  std::cerr << "[SilentInput] connection lost: " << (message ? message : "")
            << " (" << res << ")\n";
  s->lost = true;
}

const pw_core_events kCoreEvents = [] {
  pw_core_events events = {};
  events.version = PW_VERSION_CORE_EVENTS;
  events.error = OnCoreError;
  return events;
}();

const pw_proxy_events kNodeEvents = [] {
  pw_proxy_events events = {};
  events.version = PW_VERSION_PROXY_EVENTS;
  events.bound = OnBound;
  events.error = OnError;
  events.done = OnDone;
  return events;
}();

std::mutex lock;
Silence* current = nullptr;

void Destroy(Silence* s) {
  if (s->loop != nullptr) {
    pw_thread_loop_lock(s->loop);
    if (s->listening) spa_hook_remove(&s->node_listener);
    if (s->watching) spa_hook_remove(&s->core_listener);
    if (s->node != nullptr) pw_proxy_destroy(s->node);
    if (s->core != nullptr) pw_core_disconnect(s->core);
    pw_thread_loop_unlock(s->loop);
    pw_thread_loop_stop(s->loop);
    if (s->context != nullptr) pw_context_destroy(s->context);
    pw_thread_loop_destroy(s->loop);
  }
  delete s;
}

}  // namespace

bool SilentInput::Start() {
  std::lock_guard<std::mutex> guard(lock);
  if (current != nullptr) {
    if (!current->lost) return true;
    // Made on a connection since lost: the node is gone. Made again on a new one.
    Destroy(current);
    current = nullptr;
  }
  static const bool initialized = (pw_init(nullptr, nullptr), true);
  (void)initialized;
  auto s = new Silence();
  s->loop = pw_thread_loop_new("fogata-silence", nullptr);
  if (s->loop == nullptr) {
    delete s;
    return false;
  }
  s->context = pw_context_new(pw_thread_loop_get_loop(s->loop), nullptr, 0);
  if (s->context == nullptr || pw_thread_loop_start(s->loop) < 0) {
    Destroy(s);
    return false;
  }
  pw_thread_loop_lock(s->loop);
  s->core = pw_context_connect(s->context, nullptr, 0);
  if (s->core != nullptr) {
    pw_core_add_listener(s->core, &s->core_listener, &kCoreEvents, s);
    s->watching = true;
    // The lowest priority, so the system never picks it as its own default
    // input. Named after this process, so the application picks its own and
    // not another Fogata's, which goes away with it.
    const std::string pid = std::to_string(getpid());
    const std::string node_name = "fogata-silence-" + pid;
    const std::string description = "Fogata silence (" + pid + ")";
    pw_properties* props = pw_properties_new(
        "factory.name", "support.null-audio-sink",
        PW_KEY_NODE_NAME, node_name.c_str(),
        PW_KEY_NODE_DESCRIPTION, description.c_str(),
        PW_KEY_MEDIA_CLASS, "Audio/Source/Virtual",
        "audio.position", "MONO",
        "priority.session", "0", "priority.driver", "0",
        nullptr);
    s->node = static_cast<pw_proxy*>(
        pw_core_create_object(s->core, "adapter", PW_TYPE_INTERFACE_Node,
                              PW_VERSION_NODE, &props->dict, 0));
    pw_properties_free(props);
  }
  // pw_core_create_object only queues the request: the node exists once the
  // server says so. Without the null sink factory (an older or trimmed
  // PipeWire) it answers with an error instead, and nothing would ever show
  // up in the list of inputs.
  if (s->node != nullptr) {
    pw_proxy_add_listener(s->node, &s->node_listener, &kNodeEvents, s);
    s->listening = true;
    s->pending = pw_proxy_sync(s->node, 0);
    while (!s->answered && s->pending >= 0) {
      if (pw_thread_loop_timed_wait(s->loop, kAnswerSeconds) != 0) {
        std::cerr << "[SilentInput] no answer from PipeWire\n";
        s->failed = true;
        break;
      }
    }
    if (s->pending < 0) s->failed = true;
  }
  const bool made = s->node != nullptr && !s->failed;
  pw_thread_loop_unlock(s->loop);
  if (!made) {
    Destroy(s);
    return false;
  }
  current = s;
  return true;
}

void SilentInput::Stop() {
  std::lock_guard<std::mutex> guard(lock);
  if (current == nullptr) return;
  Destroy(current);
  current = nullptr;
}

}  // namespace flutter_webrtc_plugin

#else

namespace flutter_webrtc_plugin {

bool SilentInput::Start() {
  return false;
}

void SilentInput::Stop() {}

}  // namespace flutter_webrtc_plugin

#endif  // HAVE_PIPEWIRE
