#include "input_watcher.h"

#ifdef HAVE_LIBPULSE

#include <pulse/context.h>
#include <pulse/mainloop.h>
#include <pulse/subscribe.h>
#include <pulse/thread-mainloop.h>
#include <pulse/timeval.h>

#include <mutex>

namespace flutter_webrtc_plugin {

namespace {

struct Watcher {
  pa_threaded_mainloop* loop = nullptr;
  pa_context* context = nullptr;
  std::function<void()> changed;
};

std::mutex lock;
Watcher* current = nullptr;

void OnEvent(pa_context* /*c*/,
             pa_subscription_event_type_t type,
             uint32_t /*index*/,
             void* data) {
  const auto kind = type & PA_SUBSCRIPTION_EVENT_TYPE_MASK;
  if (kind != PA_SUBSCRIPTION_EVENT_NEW && kind != PA_SUBSCRIPTION_EVENT_REMOVE) {
    return;
  }
  // The server sends the event once its list has changed, so the list read now
  // is the new one.
  static_cast<Watcher*>(data)->changed();
}

void OnState(pa_context* c, void* data) {
  if (pa_context_get_state(c) != PA_CONTEXT_READY) return;
  pa_context_set_subscribe_callback(c, OnEvent, data);
  pa_operation* op =
      pa_context_subscribe(c, PA_SUBSCRIPTION_MASK_SOURCE, nullptr, nullptr);
  if (op != nullptr) pa_operation_unref(op);
  // Anything that changed before the watcher was listening.
  static_cast<Watcher*>(data)->changed();
}

void Destroy(Watcher* w) {
  if (w->loop != nullptr) {
    pa_threaded_mainloop_stop(w->loop);
    if (w->context != nullptr) {
      pa_context_disconnect(w->context);
      pa_context_unref(w->context);
    }
    pa_threaded_mainloop_free(w->loop);
  }
  delete w;
}

}  // namespace

void InputWatcher::Start(std::function<void()> changed) {
  std::lock_guard<std::mutex> guard(lock);
  if (current != nullptr) return;
  auto w = new Watcher();
  w->changed = std::move(changed);
  w->loop = pa_threaded_mainloop_new();
  if (w->loop == nullptr) {
    Destroy(w);
    return;
  }
  w->context = pa_context_new(pa_threaded_mainloop_get_api(w->loop),
                              "Fogata input watcher");
  if (w->context == nullptr) {
    Destroy(w);
    return;
  }
  pa_context_set_state_callback(w->context, OnState, w);
  // Never starts a sound server of its own.
  if (pa_context_connect(w->context, nullptr, PA_CONTEXT_NOAUTOSPAWN,
                         nullptr) < 0 ||
      pa_threaded_mainloop_start(w->loop) < 0) {
    Destroy(w);
    return;
  }
  current = w;
}

void InputWatcher::Stop() {
  std::lock_guard<std::mutex> guard(lock);
  if (current == nullptr) return;
  Destroy(current);
  current = nullptr;
}

namespace {

// Ends the check's loop once the context is ready or has failed.
void OnCheckState(pa_context* c, void* data) {
  const pa_context_state_t state = pa_context_get_state(c);
  if (state == PA_CONTEXT_READY || !PA_CONTEXT_IS_GOOD(state)) {
    pa_mainloop_quit(static_cast<pa_mainloop*>(data), 0);
  }
}

// Ends the check's loop when its time is up.
void OnCheckTimeout(pa_mainloop_api* /*api*/,
                    pa_time_event* /*event*/,
                    const struct timeval* /*when*/,
                    void* data) {
  pa_mainloop_quit(static_cast<pa_mainloop*>(data), 0);
}

}  // namespace

InputWatcher::Server InputWatcher::CheckServer(int seconds) {
  pa_mainloop* loop = pa_mainloop_new();
  if (loop == nullptr) return Server::kUnknown;
  pa_mainloop_api* api = pa_mainloop_get_api(loop);
  pa_context* context = pa_context_new(api, "Fogata sound check");
  if (context == nullptr) {
    pa_mainloop_free(loop);
    return Server::kUnknown;
  }
  Server answer = Server::kUnknown;
  pa_context_set_state_callback(context, OnCheckState, loop);
  if (pa_context_connect(context, nullptr, PA_CONTEXT_NOAUTOSPAWN, nullptr) <
      0) {
    // No server named or found at all.
    answer = Server::kNone;
  } else {
    struct timeval until;
    pa_gettimeofday(&until);
    pa_timeval_add(&until, static_cast<pa_usec_t>(seconds) * PA_USEC_PER_SEC);
    pa_time_event* timer = api->time_new(api, &until, OnCheckTimeout, loop);
    // Sleeps in poll() until the server answers, the connection fails or the
    // timer fires.
    pa_mainloop_run(loop, nullptr);
    if (timer != nullptr) api->time_free(timer);
    const pa_context_state_t state = pa_context_get_state(context);
    if (state == PA_CONTEXT_READY) {
      answer = Server::kReady;
    } else if (state == PA_CONTEXT_FAILED || state == PA_CONTEXT_TERMINATED) {
      answer = Server::kNone;
    } else {
      answer = Server::kTimeout;
    }
    pa_context_set_state_callback(context, nullptr, nullptr);
    pa_context_disconnect(context);
  }
  pa_context_unref(context);
  pa_mainloop_free(loop);
  return answer;
}

}  // namespace flutter_webrtc_plugin

#else

namespace flutter_webrtc_plugin {

void InputWatcher::Start(std::function<void()> /*changed*/) {}

void InputWatcher::Stop() {}

InputWatcher::Server InputWatcher::CheckServer(int /*seconds*/) {
  return Server::kUnknown;
}

}  // namespace flutter_webrtc_plugin

#endif  // HAVE_LIBPULSE
