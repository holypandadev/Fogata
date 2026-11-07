#include "output_keeper.h"

#ifdef HAVE_LIBPULSE

#include <pulse/context.h>
#include <pulse/introspect.h>
#include <pulse/proplist.h>
#include <pulse/subscribe.h>
#include <pulse/thread-mainloop.h>
#include <unistd.h>

#include <mutex>

namespace flutter_webrtc_plugin {

namespace {

struct Keeper {
  pa_threaded_mainloop* loop = nullptr;
  pa_context* context = nullptr;
  // Read and changed with the loop's lock held.
  std::string output;
  bool ready = false;
};

std::mutex lock;
Keeper* current = nullptr;

// Moves this process's streams that play elsewhere to the sink at `sink`.
struct Move {
  Keeper* keeper;
  uint32_t sink;
};

void OnInput(pa_context* c, const pa_sink_input_info* info, int eol, void* data) {
  auto* move = static_cast<Move*>(data);
  if (eol != 0) {
    delete move;
    return;
  }
  if (info == nullptr || info->sink == move->sink) return;
  const char* pid = pa_proplist_gets(info->proplist, PA_PROP_APPLICATION_PROCESS_ID);
  if (pid == nullptr || std::to_string(getpid()) != pid) return;
  pa_operation* op =
      pa_context_move_sink_input_by_index(c, info->index, move->sink, nullptr, nullptr);
  if (op != nullptr) pa_operation_unref(op);
}

void OnSink(pa_context* c, const pa_sink_info* info, int eol, void* data) {
  auto* keeper = static_cast<Keeper*>(data);
  if (eol != 0 || info == nullptr || keeper->output.empty()) return;
  const bool chosen = (info->description != nullptr && keeper->output == info->description) ||
                      (info->name != nullptr && keeper->output == info->name);
  if (!chosen) return;
  pa_operation* op = pa_context_get_sink_input_info_list(
      c, OnInput, new Move{keeper, info->index});
  if (op != nullptr) pa_operation_unref(op);
}

// Looks the chosen output up and moves what plays elsewhere to it. Runs on the loop's
// thread, or with its lock held.
void KeepAll(Keeper* keeper) {
  if (!keeper->ready || keeper->output.empty()) return;
  pa_operation* op = pa_context_get_sink_info_list(keeper->context, OnSink, keeper);
  if (op != nullptr) pa_operation_unref(op);
}

void OnEvent(pa_context*, pa_subscription_event_type_t type, uint32_t, void* data) {
  // A new stream, or a new output (the chosen one plugged in again).
  if ((type & PA_SUBSCRIPTION_EVENT_TYPE_MASK) != PA_SUBSCRIPTION_EVENT_NEW) return;
  KeepAll(static_cast<Keeper*>(data));
}

void OnState(pa_context* c, void* data) {
  if (pa_context_get_state(c) != PA_CONTEXT_READY) return;
  auto* keeper = static_cast<Keeper*>(data);
  keeper->ready = true;
  pa_context_set_subscribe_callback(c, OnEvent, keeper);
  pa_operation* op = pa_context_subscribe(
      c, static_cast<pa_subscription_mask_t>(PA_SUBSCRIPTION_MASK_SINK_INPUT |
                                             PA_SUBSCRIPTION_MASK_SINK),
      nullptr, nullptr);
  if (op != nullptr) pa_operation_unref(op);
  KeepAll(keeper);
}

void Destroy(Keeper* k) {
  if (k->loop != nullptr) {
    pa_threaded_mainloop_stop(k->loop);
    if (k->context != nullptr) {
      pa_context_disconnect(k->context);
      pa_context_unref(k->context);
    }
    pa_threaded_mainloop_free(k->loop);
  }
  delete k;
}

Keeper* Start() {
  auto* k = new Keeper();
  k->loop = pa_threaded_mainloop_new();
  if (k->loop == nullptr) {
    Destroy(k);
    return nullptr;
  }
  k->context =
      pa_context_new(pa_threaded_mainloop_get_api(k->loop), "Fogata output keeper");
  if (k->context == nullptr) {
    Destroy(k);
    return nullptr;
  }
  pa_context_set_state_callback(k->context, OnState, k);
  // Never starts a sound server of its own.
  if (pa_context_connect(k->context, nullptr, PA_CONTEXT_NOAUTOSPAWN, nullptr) < 0 ||
      pa_threaded_mainloop_start(k->loop) < 0) {
    Destroy(k);
    return nullptr;
  }
  return k;
}

}  // namespace

void OutputKeeper::SendTo(const std::string& output) {
  std::lock_guard<std::mutex> guard(lock);
  if (current == nullptr) {
    // Nothing to keep yet: the default is where sounds go anyway.
    if (output.empty()) return;
    current = Start();
    if (current == nullptr) return;
  }
  pa_threaded_mainloop_lock(current->loop);
  current->output = output;
  KeepAll(current);
  pa_threaded_mainloop_unlock(current->loop);
}

}  // namespace flutter_webrtc_plugin

#else

namespace flutter_webrtc_plugin {

void OutputKeeper::SendTo(const std::string& /*output*/) {}

}  // namespace flutter_webrtc_plugin

#endif  // HAVE_LIBPULSE
