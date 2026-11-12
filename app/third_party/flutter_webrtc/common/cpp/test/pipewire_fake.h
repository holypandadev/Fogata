// A PipeWire server played by the test itself, for the plugin's PipeWire code
// (linux/pipewire_apps_capturer.cc, linux/silent_input.cc) built against libpipewire's own
// headers and linked with pipewire_fake.cc in place of libpipewire. No daemon is reached.
//
// What the plugin counts on is kept: a thread loop is a thread with a lock, held while the
// connection's events run on it (as pw_thread_loop does), and taken by the plugin around
// every call it makes from another thread; proxies, the registry, nodes bound from it, and
// streams take listeners as PipeWire's do and stop getting events once destroyed. The
// server's side is the test's: it posts events to a connection's loop, which run there in
// order, and reads back what the plugin asked for (links, nodes). Nothing else a real
// server does is imitated.

#ifndef FLUTTER_WEBRTC_TEST_PIPEWIRE_FAKE_H_
#define FLUTTER_WEBRTC_TEST_PIPEWIRE_FAKE_H_

#include <pipewire/pipewire.h>

#include <cstdint>
#include <functional>
#include <set>
#include <string>
#include <utility>
#include <vector>

namespace pwfake {

using Props = std::vector<std::pair<std::string, std::string>>;

// The newest connection made (pw_context_connect), or null.
pw_core* NewestCore();

// Runs `event` on `core`'s loop thread with the loop's lock held, `delay_ms` from now, after
// the events posted before it that are due; never once the loop stopped.
void Post(pw_core* core, std::function<void()> event, int delay_ms = 0);

// Waits until every event posted to `core`'s loop so far has run, or the loop stopped.
void Drain(pw_core* core);

// The server's side, from posted events (on the loop thread, with its lock held).
void Global(pw_core* core, uint32_t id, const char* type, const Props& props);
void GlobalRemove(pw_core* core, uint32_t id);
// A node's info with these properties (every change flagged), to what is bound to it.
void NodeInfo(pw_core* core, uint32_t id, const Props& props);
// The connection's own error, as when the server goes away; the nodes made on it go.
void LoseConnection(pw_core* core);

// What the plugin did on `core`, read from any thread.
// The links not destroyed: (output port, input port).
std::set<std::pair<uint32_t, uint32_t>> Links(pw_core* core);
// Every link asked for so far.
int LinksMade(pw_core* core);
// The node of the newest stream connected on it, or SPA_ID_INVALID.
uint32_t StreamNode(pw_core* core);
bool Disconnected(pw_core* core);

// Nodes made with the adapter factory (the silent input) and not destroyed, on
// connections not lost; on every connection.
int NodesAlive();

// How the server answers the sync after a node is made with the adapter factory: bound
// and done, an error and done, or never; `delay_ms` after the sync.
enum class Answer { kBound, kError, kNone };
void SetAnswer(Answer answer, int delay_ms = 0);

// Whether the server takes connections.
void SetReachable(bool reachable);

}  // namespace pwfake

#endif  // FLUTTER_WEBRTC_TEST_PIPEWIRE_FAKE_H_
