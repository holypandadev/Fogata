#include "pipewire_apps_capturer.h"

#ifdef HAVE_PIPEWIRE

#include <pipewire/pipewire.h>
#include <spa/param/audio/format-utils.h>
#include <unistd.h>

#include <climits>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <map>
#include <memory>
#include <string>
#include <utility>
#include <vector>

namespace flutter_webrtc_plugin {

namespace {

constexpr int kRate = 48000;
constexpr int kChannels = 2;
// WebRTC takes sound in 10 ms frames.
constexpr size_t kFrame = kRate / 100;

// An executable's name without the " (deleted)" the kernel adds once the file was
// replaced on disk (an update while the app runs), so the running copy still
// matches the name a sound stream reports, whichever of the two forms it took.
std::string BinaryName(std::string name) {
  static const std::string kDeleted = " (deleted)";
  if (name.size() > kDeleted.size() &&
      name.compare(name.size() - kDeleted.size(), kDeleted.size(), kDeleted) ==
          0) {
    name.resize(name.size() - kDeleted.size());
  }
  return name;
}

// This application's executable name, as PipeWire reports it: its own sound,
// and that of any other copy of it, is left out.
std::string OwnBinary() {
  char path[PATH_MAX];
  const ssize_t n = readlink("/proc/self/exe", path, sizeof(path) - 1);
  if (n <= 0) return "";
  path[n] = 0;
  const char* slash = std::strrchr(path, '/');
  return BinaryName(slash != nullptr ? slash + 1 : path);
}

// A process id from a property, or 0 when it is missing or not a number.
int PidOf(const char* value) {
  if (value == nullptr) return 0;
  char* end = nullptr;
  const long pid = std::strtol(value, &end, 10);
  if (end == value || *end != 0 || pid <= 0 || pid > INT_MAX) return 0;
  return int(pid);
}

// An object id from a property, or SPA_ID_INVALID.
uint32_t IdOf(const char* value) {
  if (value == nullptr) return SPA_ID_INVALID;
  char* end = nullptr;
  const unsigned long id = std::strtoul(value, &end, 10);
  if (end == value || *end != 0 || id >= SPA_ID_INVALID) return SPA_ID_INVALID;
  return uint32_t(id);
}

// With FOGATA_CALL_LOG set (the switch of the call's own log), each decision about
// an application's output goes to standard error, so a sighting of the app's own
// sound in a shared screen can be explained: which node, the process and binary
// it gave, its client's process, and whether it was linked.
bool Tracing() {
  static const bool tracing = std::getenv("FOGATA_CALL_LOG") != nullptr;
  return tracing;
}

// For tests: FOGATA_SCREEN_SOUND_FROM names the one program whose sound is taken,
// so a test never takes the sound of the person's own applications. Debug builds only.
std::string OnlyFrom() {
#ifdef NDEBUG
  return "";
#else
  const char* only = std::getenv("FOGATA_SCREEN_SOUND_FROM");
  return only != nullptr ? only : "";
#endif
}

struct Node {
  bool app_output = false;
  // Whose it is comes from the node's full properties, a moment after it appears,
  // and from its client's process as the server saw it on the socket. Until a
  // process or a binary is actually known the node is not linked: a stream that
  // says nothing about itself may be this app's own call sound.
  bool known = false;
  // Once seen to be this app's own a node stays so: properties that come later
  // and say less never get it linked.
  bool own = false;
  // Whether it was linked yet, for the trace.
  bool linked = false;
  uint32_t client = SPA_ID_INVALID;
  int pid = 0;
  std::string binary;
};

struct Port {
  uint32_t node = 0;
  bool output = false;
  std::string channel;
};

struct Watched;

}  // namespace

struct PipeWireAppsCapturer::State {
  scoped_refptr<RTCAudioSource> source;
  std::string own_binary = OwnBinary();
  std::string only_from = OnlyFrom();
  pid_t own_pid = getpid();

  pw_thread_loop* loop = nullptr;
  pw_context* context = nullptr;
  pw_core* core = nullptr;
  pw_registry* registry = nullptr;
  spa_hook registry_listener{};
  pw_stream* stream = nullptr;
  spa_hook stream_listener{};
  uint32_t node = SPA_ID_INVALID;

  std::map<uint32_t, Node> nodes;
  // Each client's process id, as the server took it from the client's socket
  // (pipewire.sec.pid), by client id.
  std::map<uint32_t, int> clients;
  std::map<uint32_t, std::unique_ptr<Watched>> watched;
  std::map<uint32_t, Port> ports;
  // The links this capture made, by the output and input port they join; each
  // goes when either port does, so a long share does not pile them up.
  std::map<std::pair<uint32_t, uint32_t>, pw_proxy*> links;

  std::vector<int16_t> pending;
};

namespace {

using State = PipeWireAppsCapturer::State;

// An application's output, bound to read its full properties.
struct Watched {
  State* state = nullptr;
  uint32_t id = 0;
  pw_proxy* proxy = nullptr;
  spa_hook listener{};

  ~Watched() {
    spa_hook_remove(&listener);
    pw_proxy_destroy(proxy);
  }
};

void Relink(State* s);

// Takes away every link this capture made from a node's ports.
void Unlink(State* s, uint32_t node) {
  for (auto it = s->links.begin(); it != s->links.end();) {
    auto port = s->ports.find(it->first.first);
    if (port != s->ports.end() && port->second.node == node) {
      if (it->second != nullptr) pw_proxy_destroy(it->second);
      it = s->links.erase(it);
    } else {
      ++it;
    }
  }
}

// Decides again whose a node is, from all that is known of it now. A node found to
// be this app's own loses the links it may already have, so a stream whose
// process was told late is not left in the shared screen's sound.
void Judge(State* s, uint32_t id, Node& node) {
  if (!node.app_output) return;
  int client_pid = 0;
  if (node.client != SPA_ID_INVALID) {
    auto client = s->clients.find(node.client);
    if (client != s->clients.end()) client_pid = client->second;
  }
  const std::string binary = BinaryName(node.binary);
  const bool own =
      (node.pid != 0 && node.pid == s->own_pid) ||
      (client_pid != 0 && client_pid == s->own_pid) ||
      (!binary.empty() && !s->own_binary.empty() && s->own_binary == binary) ||
      (!s->only_from.empty() && s->only_from != binary);
  const bool was_own = node.own;
  node.own = node.own || own;
  // The client's process alone does not make a node known: every stream of a
  // PulseAudio application belongs to the one pipewire-pulse client.
  node.known = node.known || node.pid != 0 || !binary.empty() || node.own;
  if (node.own && !was_own) Unlink(s, id);
  if (Tracing()) {
    std::fprintf(stderr,
                 "[ScreenSound] node %u: pid=%d binary=%s client=%d "
                 "client_pid=%d own_pid=%d -> %s\n",
                 id, node.pid, binary.empty() ? "?" : binary.c_str(),
                 node.client == SPA_ID_INVALID ? -1 : int(node.client),
                 client_pid, int(s->own_pid),
                 node.own     ? "own, not linked"
                 : node.known ? "taken"
                              : "unknown, not linked");
  }
}

void OnNodeInfo(void* data, const pw_node_info* info) {
  auto w = static_cast<Watched*>(data);
  State* s = w->state;
  if (info == nullptr || info->props == nullptr) return;
  if ((info->change_mask & PW_NODE_CHANGE_MASK_PROPS) == 0) return;
  auto node = s->nodes.find(w->id);
  if (node == s->nodes.end()) return;
  // What a later update leaves out is kept from before.
  if (const int pid =
          PidOf(spa_dict_lookup(info->props, PW_KEY_APP_PROCESS_ID))) {
    node->second.pid = pid;
  }
  const char* binary = spa_dict_lookup(info->props, PW_KEY_APP_PROCESS_BINARY);
  if (binary != nullptr && *binary != 0) node->second.binary = binary;
  const uint32_t client = IdOf(spa_dict_lookup(info->props, PW_KEY_CLIENT_ID));
  if (client != SPA_ID_INVALID) node->second.client = client;
  Judge(s, w->id, node->second);
  Relink(s);
}

// Links every port of every other application's output to this stream's input
// of the same channel; a mono output goes to both.
void Relink(State* s) {
  if (s->node == SPA_ID_INVALID) return;
  std::map<std::string, uint32_t> inputs;
  for (const auto& [id, port] : s->ports) {
    if (port.node == s->node && !port.output) inputs[port.channel] = id;
  }
  if (inputs.empty()) return;
  for (const auto& [id, port] : s->ports) {
    if (!port.output) continue;
    auto node = s->nodes.find(port.node);
    if (node == s->nodes.end() || !node->second.app_output ||
        !node->second.known || node->second.own) {
      continue;
    }
    if (!node->second.linked) {
      node->second.linked = true;
      if (Tracing()) {
        std::fprintf(stderr,
                     "[ScreenSound] linking node %u: pid=%d binary=%s\n",
                     port.node, node->second.pid,
                     node->second.binary.empty()
                         ? "?"
                         : node->second.binary.c_str());
      }
    }
    std::vector<uint32_t> targets;
    auto same = inputs.find(port.channel);
    if (same != inputs.end()) {
      targets.push_back(same->second);
    } else {
      for (const auto& [_, input] : inputs) targets.push_back(input);
    }
    for (uint32_t input : targets) {
      if (s->links.count({id, input}) != 0) continue;
      const std::string out_node = std::to_string(port.node);
      const std::string out_port = std::to_string(id);
      const std::string in_node = std::to_string(s->node);
      const std::string in_port = std::to_string(input);
      pw_properties* props = pw_properties_new(
          PW_KEY_LINK_OUTPUT_NODE, out_node.c_str(), PW_KEY_LINK_OUTPUT_PORT,
          out_port.c_str(), PW_KEY_LINK_INPUT_NODE, in_node.c_str(),
          PW_KEY_LINK_INPUT_PORT, in_port.c_str(), PW_KEY_OBJECT_LINGER, "false",
          nullptr);
      auto link = static_cast<pw_proxy*>(pw_core_create_object(
          s->core, "link-factory", PW_TYPE_INTERFACE_Link, PW_VERSION_LINK,
          &props->dict, 0));
      pw_properties_free(props);
      // Kept even when it could not be made, so it is not asked for again
      // and again while the ports stay.
      s->links[{id, input}] = link;
    }
  }
}

void OnGlobal(void* data, uint32_t id, uint32_t, const char* type, uint32_t,
              const spa_dict* props) {
  auto s = static_cast<State*>(data);
  if (props == nullptr) return;
  if (std::strcmp(type, PW_TYPE_INTERFACE_Node) == 0) {
    static pw_node_events node_events = [] {
      pw_node_events e{};
      e.version = PW_VERSION_NODE_EVENTS;
      e.info = OnNodeInfo;
      return e;
    }();
    const char* media_class = spa_dict_lookup(props, PW_KEY_MEDIA_CLASS);
    Node node;
    node.app_output =
        media_class != nullptr && std::strcmp(media_class, "Stream/Output/Audio") == 0;
    node.client = IdOf(spa_dict_lookup(props, PW_KEY_CLIENT_ID));
    s->nodes[id] = node;
    if (node.app_output) {
      auto proxy = static_cast<pw_proxy*>(
          pw_registry_bind(s->registry, id, PW_TYPE_INTERFACE_Node,
                           PW_VERSION_NODE, 0));
      if (proxy != nullptr) {
        auto w = std::make_unique<Watched>();
        w->state = s;
        w->id = id;
        w->proxy = proxy;
        pw_node_add_listener(reinterpret_cast<pw_node*>(proxy), &w->listener,
                             &node_events, w.get());
        s->watched[id] = std::move(w);
      }
    }
  } else if (std::strcmp(type, PW_TYPE_INTERFACE_Client) == 0) {
    // The process the server saw on the client's socket, which a stream cannot
    // misreport: a node of this app's own whose properties lack its process is
    // still told apart by it.
    const int pid = PidOf(spa_dict_lookup(props, PW_KEY_SEC_PID));
    if (pid == 0) return;
    s->clients[id] = pid;
    for (auto& [node_id, node] : s->nodes) {
      if (node.client == id) Judge(s, node_id, node);
    }
  } else if (std::strcmp(type, PW_TYPE_INTERFACE_Port) == 0) {
    const char* node = spa_dict_lookup(props, PW_KEY_NODE_ID);
    const char* direction = spa_dict_lookup(props, PW_KEY_PORT_DIRECTION);
    const char* monitor = spa_dict_lookup(props, PW_KEY_PORT_MONITOR);
    const char* channel = spa_dict_lookup(props, PW_KEY_AUDIO_CHANNEL);
    if (node == nullptr || direction == nullptr) return;
    if (monitor != nullptr && std::strcmp(monitor, "true") == 0) return;
    Port port;
    port.node = uint32_t(std::atoi(node));
    port.output = std::strcmp(direction, "out") == 0;
    port.channel = channel != nullptr ? channel : "MONO";
    s->ports[id] = port;
  } else {
    return;
  }
  Relink(s);
}

void OnGlobalRemove(void* data, uint32_t id) {
  auto s = static_cast<State*>(data);
  s->nodes.erase(id);
  s->clients.erase(id);
  s->watched.erase(id);
  s->ports.erase(id);
  // The server already dropped a link whose port went away; its proxy stays
  // until it is destroyed.
  for (auto it = s->links.begin(); it != s->links.end();) {
    if (it->first.first == id || it->first.second == id) {
      if (it->second != nullptr) pw_proxy_destroy(it->second);
      it = s->links.erase(it);
    } else {
      ++it;
    }
  }
}

void OnStateChanged(void* data, pw_stream_state, pw_stream_state state,
                    const char*) {
  auto s = static_cast<State*>(data);
  if (state == PW_STREAM_STATE_PAUSED || state == PW_STREAM_STATE_STREAMING) {
    const uint32_t node = pw_stream_get_node_id(s->stream);
    if (node != SPA_ID_INVALID && node != s->node) {
      s->node = node;
      Relink(s);
    }
  }
}

void OnProcess(void* data) {
  auto s = static_cast<State*>(data);
  pw_buffer* buffer = pw_stream_dequeue_buffer(s->stream);
  if (buffer == nullptr) return;
  spa_data* d = &buffer->buffer->datas[0];
  // The samples must lie within the buffer, and start where a sample can.
  const bool inside = d->chunk->offset <= d->maxsize &&
                      d->chunk->size <= d->maxsize - d->chunk->offset &&
                      d->chunk->offset % sizeof(int16_t) == 0;
  if (d->data != nullptr && d->chunk->size > 0 && inside) {
    auto samples = reinterpret_cast<const int16_t*>(
        static_cast<const uint8_t*>(d->data) + d->chunk->offset);
    const size_t count = d->chunk->size / sizeof(int16_t);
    s->pending.insert(s->pending.end(), samples, samples + count);
    size_t used = 0;
    while (s->pending.size() - used >= kFrame * kChannels) {
      s->source->CaptureFrame(s->pending.data() + used, 16, kRate, kChannels,
                              kFrame);
      used += kFrame * kChannels;
    }
    s->pending.erase(s->pending.begin(), s->pending.begin() + used);
  }
  pw_stream_queue_buffer(s->stream, buffer);
}

void Teardown(State* s) {
  if (s->loop == nullptr) return;
  pw_thread_loop_lock(s->loop);
  for (const auto& [_, link] : s->links) {
    if (link != nullptr) pw_proxy_destroy(link);
  }
  s->links.clear();
  s->watched.clear();
  if (s->stream != nullptr) {
    pw_stream_disconnect(s->stream);
    pw_stream_destroy(s->stream);
    s->stream = nullptr;
  }
  if (s->registry != nullptr) {
    pw_proxy_destroy(reinterpret_cast<pw_proxy*>(s->registry));
    s->registry = nullptr;
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

}  // namespace

PipeWireAppsCapturer::PipeWireAppsCapturer() : state_(std::make_unique<State>()) {}

PipeWireAppsCapturer::~PipeWireAppsCapturer() {
  Stop();
}

bool PipeWireAppsCapturer::Start(scoped_refptr<RTCAudioSource> source) {
  static const bool initialized = (pw_init(nullptr, nullptr), true);
  (void)initialized;
  static pw_registry_events registry_events = [] {
    pw_registry_events e{};
    e.version = PW_VERSION_REGISTRY_EVENTS;
    e.global = OnGlobal;
    e.global_remove = OnGlobalRemove;
    return e;
  }();
  static pw_stream_events stream_events = [] {
    pw_stream_events e{};
    e.version = PW_VERSION_STREAM_EVENTS;
    e.state_changed = OnStateChanged;
    e.process = OnProcess;
    return e;
  }();
  State* s = state_.get();
  s->source = source;
  s->loop = pw_thread_loop_new("fogata-screen-sound", nullptr);
  if (s->loop == nullptr) return false;
  s->context = pw_context_new(pw_thread_loop_get_loop(s->loop), nullptr, 0);
  if (s->context == nullptr || pw_thread_loop_start(s->loop) < 0) {
    Teardown(s);
    return false;
  }
  pw_thread_loop_lock(s->loop);
  s->core = pw_context_connect(s->context, nullptr, 0);
  if (s->core == nullptr) {
    pw_thread_loop_unlock(s->loop);
    Teardown(s);
    return false;
  }
  // Not linked by the session manager: this capture links itself to each
  // application's output.
  s->stream = pw_stream_new(
      s->core, "Fogata screen sound",
      pw_properties_new(PW_KEY_MEDIA_TYPE, "Audio", PW_KEY_MEDIA_CATEGORY,
                        "Capture", PW_KEY_MEDIA_ROLE, "Screen",
                        PW_KEY_NODE_NAME, "fogata-screen-sound", nullptr));
  pw_stream_add_listener(s->stream, &s->stream_listener, &stream_events, s);
  uint8_t buffer[512];
  spa_pod_builder b = SPA_POD_BUILDER_INIT(buffer, sizeof(buffer));
  spa_audio_info_raw info{};
  info.format = SPA_AUDIO_FORMAT_S16;
  info.rate = kRate;
  info.channels = kChannels;
  info.position[0] = SPA_AUDIO_CHANNEL_FL;
  info.position[1] = SPA_AUDIO_CHANNEL_FR;
  const spa_pod* params[1] = {
      spa_format_audio_raw_build(&b, SPA_PARAM_EnumFormat, &info)};
  const int connected = pw_stream_connect(
      s->stream, PW_DIRECTION_INPUT, PW_ID_ANY,
      static_cast<pw_stream_flags>(PW_STREAM_FLAG_MAP_BUFFERS |
                                   PW_STREAM_FLAG_RT_PROCESS),
      params, 1);
  s->registry = pw_core_get_registry(s->core, PW_VERSION_REGISTRY, 0);
  pw_registry_add_listener(s->registry, &s->registry_listener, &registry_events,
                           s);
  pw_thread_loop_unlock(s->loop);
  if (connected < 0) {
    Teardown(s);
    return false;
  }
  return true;
}

void PipeWireAppsCapturer::Stop() {
  Teardown(state_.get());
}

bool PipeWireAppsCapturer::Reachable() {
  static const bool initialized = (pw_init(nullptr, nullptr), true);
  (void)initialized;
  pw_main_loop* loop = pw_main_loop_new(nullptr);
  if (loop == nullptr) return false;
  bool reachable = false;
  pw_context* context = pw_context_new(pw_main_loop_get_loop(loop), nullptr, 0);
  if (context != nullptr) {
    pw_core* core = pw_context_connect(context, nullptr, 0);
    if (core != nullptr) {
      reachable = true;
      pw_core_disconnect(core);
    }
    pw_context_destroy(context);
  }
  pw_main_loop_destroy(loop);
  return reachable;
}

}  // namespace flutter_webrtc_plugin

#endif  // HAVE_PIPEWIRE
