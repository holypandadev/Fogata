// The PipeWire server of the tests (see pipewire_fake.h): libpipewire's functions the
// plugin calls, for objects of the test's own.

#include "pipewire_fake.h"

#include <cerrno>
#include <chrono>
#include <condition_variable>
#include <cstdarg>
#include <cstddef>
#include <cstdlib>
#include <cstring>
#include <map>
#include <memory>
#include <mutex>
#include <thread>

using Clock = std::chrono::steady_clock;

// The objects libpipewire keeps opaque, as the test's. Each one the plugin calls methods
// of through its interface (spa_api_method_r) starts with that interface.

struct pw_thread_loop {
  pw_loop loop{};
  std::recursive_mutex mutex;
  // The loop's thread waits on `wake` for events; pw_thread_loop_timed_wait on
  // `signal`; Drain on `idle`.
  std::condition_variable_any wake;
  std::condition_variable_any signal;
  std::condition_variable_any idle;
  std::multimap<Clock::time_point, std::function<void()>> events;
  bool stopping = false;
  std::thread thread;
};

struct pw_main_loop {
  pw_loop loop{};
};

struct pw_context {
  pw_thread_loop* loop = nullptr;
};

struct pw_core {
  spa_interface iface{};
  pw_thread_loop* loop = nullptr;
  spa_hook_list listeners{};
  bool disconnected = false;
  bool lost = false;
  std::vector<pw_proxy*> proxies;
  // The proxies bound to each global, by its id.
  std::multimap<uint32_t, pw_proxy*> bound;
  int links_made = 0;
  uint32_t stream_node = SPA_ID_INVALID;
};

struct pw_proxy {
  spa_interface iface{};
  enum Kind { kRegistry, kNode, kLink, kAdapter } kind = kNode;
  pw_core* core = nullptr;
  uint32_t global = SPA_ID_INVALID;
  // The registry's or the node's own events; the proxy's (bound, done, error).
  spa_hook_list object_listeners{};
  spa_hook_list proxy_listeners{};
  bool destroyed = false;
  uint32_t link_output = 0;
  uint32_t link_input = 0;
};

struct pw_stream {
  pw_core* core = nullptr;
  spa_hook_list listeners{};
  uint32_t node = SPA_ID_INVALID;
  bool destroyed = false;
};

namespace {

struct Properties {
  pw_properties props{};
  std::vector<std::string> strings;
  std::vector<spa_dict_item> items;
};

// What is not one connection's: under `lock`, never taken before a loop's lock.
std::mutex lock;
pw_core* newest = nullptr;
std::map<pw_loop*, pw_thread_loop*> loops;
std::map<pw_properties*, std::unique_ptr<Properties>> properties;
pwfake::Answer answer = pwfake::Answer::kBound;
int answer_delay_ms = 0;
bool reachable = true;
int next_seq = 0;
uint32_t next_node = 1000;
int adapters_alive = 0;

// Calls each hook of `list` with its events, as PipeWire emits them; a hook may be removed
// meanwhile.
template <typename Events, typename Call>
void Emit(spa_hook_list* list, Call call) {
  spa_list* head = &list->list;
  for (spa_list* link = head->next; link != head;) {
    spa_list* next = link->next;
    auto hook = reinterpret_cast<spa_hook*>(reinterpret_cast<char*>(link) -
                                            offsetof(spa_hook, link));
    if (hook->cb.funcs != nullptr) {
      call(static_cast<const Events*>(hook->cb.funcs), hook->cb.data);
    }
    link = next;
  }
}

// Takes every hook off `list`, as destroying a proxy does.
void Clean(spa_hook_list* list) {
  spa_list* head = &list->list;
  while (head->next != head) {
    auto hook = reinterpret_cast<spa_hook*>(reinterpret_cast<char*>(head->next) -
                                            offsetof(spa_hook, link));
    spa_hook_remove(hook);
    // Removed again by its owner later, it unlinks nothing.
    spa_list_init(&hook->link);
  }
}

struct Dict {
  std::vector<spa_dict_item> items;
  spa_dict dict{};
  explicit Dict(const pwfake::Props& props) {
    for (const auto& [key, value] : props) {
      items.push_back(spa_dict_item{key.c_str(), value.c_str()});
    }
    dict.flags = 0;
    dict.n_items = uint32_t(items.size());
    dict.items = items.data();
  }
};

pw_proxy* NewProxy(pw_core* core, pw_proxy::Kind kind, const char* type,
                   const void* methods) {
  auto proxy = new pw_proxy();
  proxy->kind = kind;
  proxy->core = core;
  proxy->iface.type = type;
  proxy->iface.version = 0;
  proxy->iface.cb.funcs = methods;
  proxy->iface.cb.data = proxy;
  spa_hook_list_init(&proxy->object_listeners);
  spa_hook_list_init(&proxy->proxy_listeners);
  core->proxies.push_back(proxy);
  return proxy;
}

int NodeAddListener(void* object, spa_hook* listener, const pw_node_events* events,
                    void* data) {
  auto proxy = static_cast<pw_proxy*>(object);
  spa_hook_list_append(&proxy->object_listeners, listener, events, data);
  return 0;
}

const pw_node_methods kNodeMethods = [] {
  pw_node_methods methods{};
  methods.version = PW_VERSION_NODE_METHODS;
  methods.add_listener = NodeAddListener;
  return methods;
}();

int RegistryAddListener(void* object, spa_hook* listener,
                        const pw_registry_events* events, void* data) {
  auto proxy = static_cast<pw_proxy*>(object);
  spa_hook_list_append(&proxy->object_listeners, listener, events, data);
  return 0;
}

void* RegistryBind(void* object, uint32_t id, const char*, uint32_t, size_t) {
  auto registry = static_cast<pw_proxy*>(object);
  pw_proxy* node = NewProxy(registry->core, pw_proxy::kNode, PW_TYPE_INTERFACE_Node,
                            &kNodeMethods);
  node->global = id;
  registry->core->bound.emplace(id, node);
  return node;
}

int RegistryDestroy(void*, uint32_t) {
  return 0;
}

const pw_registry_methods kRegistryMethods = [] {
  pw_registry_methods methods{};
  methods.version = PW_VERSION_REGISTRY_METHODS;
  methods.add_listener = RegistryAddListener;
  methods.bind = RegistryBind;
  methods.destroy = RegistryDestroy;
  return methods;
}();

int CoreAddListener(void* object, spa_hook* listener, const pw_core_events* events,
                    void* data) {
  auto core = static_cast<pw_core*>(object);
  spa_hook_list_append(&core->listeners, listener, events, data);
  return 0;
}

pw_registry* CoreGetRegistry(void* object, uint32_t, size_t) {
  auto core = static_cast<pw_core*>(object);
  return reinterpret_cast<pw_registry*>(NewProxy(
      core, pw_proxy::kRegistry, PW_TYPE_INTERFACE_Registry, &kRegistryMethods));
}

uint32_t PortOf(const spa_dict* props, const char* key) {
  const char* value = props != nullptr ? spa_dict_lookup(props, key) : nullptr;
  return value != nullptr ? uint32_t(std::strtoul(value, nullptr, 10)) : 0;
}

void* CoreCreateObject(void* object, const char* factory, const char*, uint32_t,
                       const spa_dict* props, size_t) {
  auto core = static_cast<pw_core*>(object);
  if (std::strcmp(factory, "link-factory") == 0) {
    pw_proxy* link = NewProxy(core, pw_proxy::kLink, PW_TYPE_INTERFACE_Link, nullptr);
    link->link_output = PortOf(props, PW_KEY_LINK_OUTPUT_PORT);
    link->link_input = PortOf(props, PW_KEY_LINK_INPUT_PORT);
    core->links_made++;
    return link;
  }
  if (std::strcmp(factory, "adapter") == 0) {
    pw_proxy* node = NewProxy(core, pw_proxy::kAdapter, PW_TYPE_INTERFACE_Node,
                              &kNodeMethods);
    std::lock_guard<std::mutex> guard(lock);
    adapters_alive++;
    return node;
  }
  return nullptr;
}

const pw_core_methods kCoreMethods = [] {
  pw_core_methods methods{};
  methods.version = PW_VERSION_CORE_METHODS;
  methods.add_listener = CoreAddListener;
  methods.get_registry = CoreGetRegistry;
  methods.create_object = CoreCreateObject;
  return methods;
}();

// An adapter node gone: destroyed, or its connection lost or closed. Under the loop's lock.
void AdapterGone(pw_proxy* proxy) {
  if (proxy->kind != pw_proxy::kAdapter || proxy->destroyed) return;
  std::lock_guard<std::mutex> guard(lock);
  adapters_alive--;
}

void Run(pw_thread_loop* loop) {
  std::unique_lock<std::recursive_mutex> held(loop->mutex);
  while (!loop->stopping) {
    if (loop->events.empty()) {
      loop->idle.notify_all();
      loop->wake.wait(held);
      continue;
    }
    auto first = loop->events.begin();
    if (first->first > Clock::now()) {
      loop->wake.wait_until(held, first->first);
      continue;
    }
    std::function<void()> event = std::move(first->second);
    loop->events.erase(first);
    event();
  }
  loop->idle.notify_all();
}

}  // namespace

extern "C" {

void pw_init(int*, char***) {}

pw_thread_loop* pw_thread_loop_new(const char*, const spa_dict*) {
  auto loop = new pw_thread_loop();
  std::lock_guard<std::mutex> guard(lock);
  loops[&loop->loop] = loop;
  return loop;
}

pw_loop* pw_thread_loop_get_loop(pw_thread_loop* loop) {
  return &loop->loop;
}

int pw_thread_loop_start(pw_thread_loop* loop) {
  loop->thread = std::thread(Run, loop);
  return 0;
}

void pw_thread_loop_stop(pw_thread_loop* loop) {
  {
    std::lock_guard<std::recursive_mutex> held(loop->mutex);
    loop->stopping = true;
    loop->wake.notify_all();
    loop->idle.notify_all();
  }
  if (loop->thread.joinable()) loop->thread.join();
}

// Kept: the test may still post to it, which then does nothing.
void pw_thread_loop_destroy(pw_thread_loop* loop) {
  pw_thread_loop_stop(loop);
}

void pw_thread_loop_lock(pw_thread_loop* loop) {
  loop->mutex.lock();
}

void pw_thread_loop_unlock(pw_thread_loop* loop) {
  loop->mutex.unlock();
}

int pw_thread_loop_timed_wait(pw_thread_loop* loop, int wait_max_sec) {
  // Held once by the caller, as PipeWire asks: the wait lets it go meanwhile.
  std::unique_lock<std::recursive_mutex> held(loop->mutex, std::adopt_lock);
  const std::cv_status status =
      loop->signal.wait_for(held, std::chrono::seconds(wait_max_sec));
  held.release();
  return status == std::cv_status::timeout ? ETIMEDOUT : 0;
}

void pw_thread_loop_signal(pw_thread_loop* loop, bool) {
  loop->signal.notify_all();
}

pw_main_loop* pw_main_loop_new(const spa_dict*) {
  return new pw_main_loop();
}

pw_loop* pw_main_loop_get_loop(pw_main_loop* loop) {
  return &loop->loop;
}

void pw_main_loop_destroy(pw_main_loop* loop) {
  delete loop;
}

pw_properties* pw_properties_new(const char* key, ...) {
  auto made = std::make_unique<Properties>();
  va_list args;
  va_start(args, key);
  for (const char* k = key; k != nullptr; k = va_arg(args, const char*)) {
    const char* value = va_arg(args, const char*);
    made->strings.push_back(k);
    made->strings.push_back(value != nullptr ? value : "");
  }
  va_end(args);
  for (size_t i = 0; i + 1 < made->strings.size(); i += 2) {
    made->items.push_back(
        spa_dict_item{made->strings[i].c_str(), made->strings[i + 1].c_str()});
  }
  made->props.dict.flags = 0;
  made->props.dict.n_items = uint32_t(made->items.size());
  made->props.dict.items = made->items.data();
  pw_properties* props = &made->props;
  std::lock_guard<std::mutex> guard(lock);
  properties[props] = std::move(made);
  return props;
}

void pw_properties_free(pw_properties* props) {
  std::lock_guard<std::mutex> guard(lock);
  properties.erase(props);
}

pw_context* pw_context_new(pw_loop* main_loop, pw_properties* props, size_t) {
  if (props != nullptr) pw_properties_free(props);
  auto context = new pw_context();
  std::lock_guard<std::mutex> guard(lock);
  auto found = loops.find(main_loop);
  context->loop = found != loops.end() ? found->second : nullptr;
  return context;
}

void pw_context_destroy(pw_context* context) {
  delete context;
}

pw_core* pw_context_connect(pw_context* context, pw_properties* props, size_t) {
  if (props != nullptr) pw_properties_free(props);
  std::lock_guard<std::mutex> guard(lock);
  if (!reachable || context->loop == nullptr) {
    errno = ECONNREFUSED;
    return nullptr;
  }
  auto core = new pw_core();
  core->iface.type = PW_TYPE_INTERFACE_Core;
  core->iface.version = PW_VERSION_CORE;
  core->iface.cb.funcs = &kCoreMethods;
  core->iface.cb.data = core;
  core->loop = context->loop;
  spa_hook_list_init(&core->listeners);
  newest = core;
  return core;
}

int pw_core_disconnect(pw_core* core) {
  core->disconnected = true;
  Clean(&core->listeners);
  for (pw_proxy* proxy : core->proxies) {
    if (proxy->destroyed) continue;
    if (!core->lost) AdapterGone(proxy);
    proxy->destroyed = true;
    Clean(&proxy->object_listeners);
    Clean(&proxy->proxy_listeners);
  }
  return 0;
}

void pw_proxy_add_listener(pw_proxy* proxy, spa_hook* listener,
                           const pw_proxy_events* events, void* data) {
  spa_hook_list_append(&proxy->proxy_listeners, listener, events, data);
}

void pw_proxy_destroy(pw_proxy* proxy) {
  if (proxy->destroyed) return;
  if (!proxy->core->lost) AdapterGone(proxy);
  proxy->destroyed = true;
  Clean(&proxy->object_listeners);
  Clean(&proxy->proxy_listeners);
  if (proxy->kind == pw_proxy::kNode) {
    auto range = proxy->core->bound.equal_range(proxy->global);
    for (auto it = range.first; it != range.second; ++it) {
      if (it->second == proxy) {
        proxy->core->bound.erase(it);
        break;
      }
    }
  }
}

int pw_proxy_sync(pw_proxy* proxy, int) {
  int seq = 0;
  pwfake::Answer how = pwfake::Answer::kBound;
  int delay = 0;
  {
    std::lock_guard<std::mutex> guard(lock);
    seq = ++next_seq;
    how = answer;
    delay = answer_delay_ms;
  }
  if (how == pwfake::Answer::kNone) return seq;
  pwfake::Post(
      proxy->core,
      [proxy, seq, how] {
        if (proxy->destroyed || proxy->core->disconnected || proxy->core->lost) return;
        if (how == pwfake::Answer::kBound) {
          Emit<pw_proxy_events>(&proxy->proxy_listeners,
                                [](const pw_proxy_events* events, void* data) {
                                  if (events->bound) events->bound(data, 4242);
                                });
        } else {
          Emit<pw_proxy_events>(&proxy->proxy_listeners,
                                [seq](const pw_proxy_events* events, void* data) {
                                  if (events->error)
                                    events->error(data, seq, -ENOENT, "no such factory");
                                });
        }
        Emit<pw_proxy_events>(&proxy->proxy_listeners,
                              [seq](const pw_proxy_events* events, void* data) {
                                if (events->done) events->done(data, seq);
                              });
      },
      delay);
  return seq;
}

pw_stream* pw_stream_new(pw_core* core, const char*, pw_properties* props) {
  if (props != nullptr) pw_properties_free(props);
  auto stream = new pw_stream();
  stream->core = core;
  spa_hook_list_init(&stream->listeners);
  return stream;
}

void pw_stream_add_listener(pw_stream* stream, spa_hook* listener,
                            const pw_stream_events* events, void* data) {
  spa_hook_list_append(&stream->listeners, listener, events, data);
}

int pw_stream_connect(pw_stream* stream, enum pw_direction, uint32_t,
                      enum pw_stream_flags, const spa_pod**, uint32_t) {
  {
    std::lock_guard<std::mutex> guard(lock);
    stream->node = next_node;
    next_node += 100;
  }
  stream->core->stream_node = stream->node;
  // Paused once the server made its node, as PipeWire tells.
  pwfake::Post(stream->core, [stream] {
    if (stream->destroyed) return;
    Emit<pw_stream_events>(&stream->listeners,
                           [](const pw_stream_events* events, void* data) {
                             if (events->state_changed) {
                               events->state_changed(data, PW_STREAM_STATE_CONNECTING,
                                                     PW_STREAM_STATE_PAUSED, nullptr);
                             }
                           });
  });
  return 0;
}

uint32_t pw_stream_get_node_id(pw_stream* stream) {
  return stream->node;
}

int pw_stream_disconnect(pw_stream*) {
  return 0;
}

void pw_stream_destroy(pw_stream* stream) {
  stream->destroyed = true;
  Clean(&stream->listeners);
}

pw_buffer* pw_stream_dequeue_buffer(pw_stream*) {
  return nullptr;
}

int pw_stream_queue_buffer(pw_stream*, pw_buffer*) {
  return 0;
}

}  // extern "C"

namespace pwfake {

pw_core* NewestCore() {
  std::lock_guard<std::mutex> guard(lock);
  return newest;
}

void Post(pw_core* core, std::function<void()> event, int delay_ms) {
  pw_thread_loop* loop = core->loop;
  std::lock_guard<std::recursive_mutex> held(loop->mutex);
  if (loop->stopping) return;
  loop->events.emplace(Clock::now() + std::chrono::milliseconds(delay_ms),
                       std::move(event));
  loop->wake.notify_all();
}

void Drain(pw_core* core) {
  pw_thread_loop* loop = core->loop;
  std::unique_lock<std::recursive_mutex> held(loop->mutex);
  loop->idle.wait(held, [loop] { return loop->stopping || loop->events.empty(); });
}

void Global(pw_core* core, uint32_t id, const char* type, const Props& props) {
  Dict dict(props);
  for (pw_proxy* proxy : std::vector<pw_proxy*>(core->proxies)) {
    if (proxy->kind != pw_proxy::kRegistry || proxy->destroyed) continue;
    Emit<pw_registry_events>(
        &proxy->object_listeners, [&](const pw_registry_events* events, void* data) {
          if (events->global) {
            events->global(data, id, PW_PERM_ALL, type, 3, &dict.dict);
          }
        });
  }
}

void GlobalRemove(pw_core* core, uint32_t id) {
  for (pw_proxy* proxy : std::vector<pw_proxy*>(core->proxies)) {
    if (proxy->kind != pw_proxy::kRegistry || proxy->destroyed) continue;
    Emit<pw_registry_events>(&proxy->object_listeners,
                             [id](const pw_registry_events* events, void* data) {
                               if (events->global_remove) events->global_remove(data, id);
                             });
  }
}

void NodeInfo(pw_core* core, uint32_t id, const Props& props) {
  Dict dict(props);
  pw_node_info info{};
  info.id = id;
  info.change_mask = PW_NODE_CHANGE_MASK_ALL;
  info.props = &dict.dict;
  std::vector<pw_proxy*> bound;
  auto range = core->bound.equal_range(id);
  for (auto it = range.first; it != range.second; ++it) bound.push_back(it->second);
  for (pw_proxy* proxy : bound) {
    if (proxy->destroyed) continue;
    Emit<pw_node_events>(&proxy->object_listeners,
                         [&info](const pw_node_events* events, void* data) {
                           if (events->info) events->info(data, &info);
                         });
  }
}

void LoseConnection(pw_core* core) {
  if (core->lost || core->disconnected) return;
  for (pw_proxy* proxy : core->proxies) AdapterGone(proxy);
  core->lost = true;
  Emit<pw_core_events>(&core->listeners, [](const pw_core_events* events, void* data) {
    if (events->error) events->error(data, PW_ID_CORE, 0, -EPIPE, "connection lost");
  });
}

std::set<std::pair<uint32_t, uint32_t>> Links(pw_core* core) {
  std::lock_guard<std::recursive_mutex> held(core->loop->mutex);
  std::set<std::pair<uint32_t, uint32_t>> links;
  for (pw_proxy* proxy : core->proxies) {
    if (proxy->kind == pw_proxy::kLink && !proxy->destroyed) {
      links.emplace(proxy->link_output, proxy->link_input);
    }
  }
  return links;
}

int LinksMade(pw_core* core) {
  std::lock_guard<std::recursive_mutex> held(core->loop->mutex);
  return core->links_made;
}

uint32_t StreamNode(pw_core* core) {
  std::lock_guard<std::recursive_mutex> held(core->loop->mutex);
  return core->stream_node;
}

bool Disconnected(pw_core* core) {
  std::lock_guard<std::recursive_mutex> held(core->loop->mutex);
  return core->disconnected;
}

int NodesAlive() {
  std::lock_guard<std::mutex> guard(lock);
  return adapters_alive;
}

void SetAnswer(Answer how, int delay_ms) {
  std::lock_guard<std::mutex> guard(lock);
  answer = how;
  answer_delay_ms = delay_ms;
}

void SetReachable(bool taken) {
  std::lock_guard<std::mutex> guard(lock);
  reachable = taken;
}

}  // namespace pwfake
