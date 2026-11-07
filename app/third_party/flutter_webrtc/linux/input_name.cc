#include "input_name.h"

#ifdef HAVE_LIBPULSE

#include <pulse/context.h>
#include <pulse/introspect.h>
#include <pulse/mainloop.h>

#include <chrono>
#include <thread>

namespace flutter_webrtc_plugin {

namespace {

struct Query {
  std::string input;
  std::string name;
  bool done = false;
};

void OnSource(pa_context*, const pa_source_info* info, int eol, void* data) {
  auto* q = static_cast<Query*>(data);
  if (eol != 0) {
    q->done = true;
    return;
  }
  if (info == nullptr || info->name == nullptr || !q->name.empty()) return;
  const bool described = info->description != nullptr && q->input == info->description;
  if (described || q->input == info->name) q->name = info->name;
}

}  // namespace

std::string InputName(const std::string& input) {
  if (input.empty()) return std::string();
  pa_mainloop* loop = pa_mainloop_new();
  if (loop == nullptr) return std::string();
  pa_context* context = pa_context_new(pa_mainloop_get_api(loop), "Fogata input name");
  Query query{input};
  // Never starts a sound server of its own.
  if (context != nullptr &&
      pa_context_connect(context, nullptr, PA_CONTEXT_NOAUTOSPAWN, nullptr) >= 0) {
    pa_operation* op = nullptr;
    const auto until = std::chrono::steady_clock::now() + std::chrono::seconds(1);
    while (!query.done && std::chrono::steady_clock::now() < until) {
      pa_mainloop_iterate(loop, 0, nullptr);
      const pa_context_state_t state = pa_context_get_state(context);
      if (!PA_CONTEXT_IS_GOOD(state)) break;
      if (state == PA_CONTEXT_READY && op == nullptr) {
        op = pa_context_get_source_info_list(context, OnSource, &query);
        if (op == nullptr) break;
      }
      if (!query.done) std::this_thread::sleep_for(std::chrono::milliseconds(2));
    }
    if (op != nullptr) {
      pa_operation_cancel(op);
      pa_operation_unref(op);
    }
    pa_context_disconnect(context);
  }
  if (context != nullptr) pa_context_unref(context);
  pa_mainloop_free(loop);
  return query.done ? query.name : std::string();
}

}  // namespace flutter_webrtc_plugin

#else

namespace flutter_webrtc_plugin {

std::string InputName(const std::string& /*input*/) { return std::string(); }

}  // namespace flutter_webrtc_plugin

#endif  // HAVE_LIBPULSE
