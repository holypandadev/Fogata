#include "input_watcher.h"

#ifdef HAVE_LIBPULSE

#include <pulse/context.h>
#include <pulse/mainloop.h>
#include <pulse/subscribe.h>
#include <pulse/thread-mainloop.h>
#include <pulse/timeval.h>

#include <algorithm>
#include <mutex>

namespace flutter_webrtc_plugin {

namespace {

struct Watcher {
  pa_threaded_mainloop* loop = nullptr;
  pa_context* context = nullptr;
  std::function<void()> changed;
  // Seconds before the next try to connect once the connection was lost (the sound
  // server restarted, as after an update): doubled after each failure, up to
  // kMostSeconds, and set back once connected. On the loop's thread.
  int retry_seconds = kFirstSeconds;
  static constexpr int kFirstSeconds = 1;
  static constexpr int kMostSeconds = 30;
};

void Connect(Watcher* w);

// On the loop's thread, a while after the connection was lost: a new one.
void OnRetry(pa_mainloop_api* api, pa_time_event* event, const struct timeval*,
             void* data) {
  api->time_free(event);
  Connect(static_cast<Watcher*>(data));
}

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
  auto* w = static_cast<Watcher*>(data);
  const pa_context_state_t state = pa_context_get_state(c);
  if (state == PA_CONTEXT_FAILED || state == PA_CONTEXT_TERMINATED) {
    // The server went away: a new connection a while later (this one cannot
    // connect again), which once ready tells what changed meanwhile.
    struct timeval when;
    pa_timeval_add(pa_gettimeofday(&when),
                   pa_usec_t(w->retry_seconds) * PA_USEC_PER_SEC);
    w->retry_seconds = std::min(w->retry_seconds * 2, Watcher::kMostSeconds);
    pa_mainloop_api* api = pa_threaded_mainloop_get_api(w->loop);
    api->time_new(api, &when, OnRetry, w);
    return;
  }
  if (state != PA_CONTEXT_READY) return;
  w->retry_seconds = Watcher::kFirstSeconds;
  pa_context_set_subscribe_callback(c, OnEvent, data);
  pa_operation* op =
      pa_context_subscribe(c, PA_SUBSCRIPTION_MASK_SOURCE, nullptr, nullptr);
  if (op != nullptr) pa_operation_unref(op);
  // Anything that changed before the watcher was listening.
  static_cast<Watcher*>(data)->changed();
}

// Lets go of the watcher's connection, if any, without telling OnState.
void Disconnect(Watcher* w) {
  if (w->context == nullptr) return;
  pa_context_set_state_callback(w->context, nullptr, nullptr);
  pa_context_set_subscribe_callback(w->context, nullptr, nullptr);
  pa_context_disconnect(w->context);
  pa_context_unref(w->context);
  w->context = nullptr;
}

// Connects to the sound server, in place of a connection lost. On the loop's thread,
// or before it runs. A failure reaches OnState, which tries again a while later.
void Connect(Watcher* w) {
  Disconnect(w);
  w->context = pa_context_new(pa_threaded_mainloop_get_api(w->loop),
                              "Fogata input watcher");
  if (w->context == nullptr) return;
  pa_context_set_state_callback(w->context, OnState, w);
  // Never starts a sound server of its own.
  pa_context_connect(w->context, nullptr, PA_CONTEXT_NOAUTOSPAWN, nullptr);
}

void Destroy(Watcher* w) {
  if (w->loop != nullptr) {
    pa_threaded_mainloop_stop(w->loop);
    Disconnect(w);
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
  Connect(w);
  if (w->context == nullptr || pa_threaded_mainloop_start(w->loop) < 0) {
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
