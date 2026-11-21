#ifndef FOGATA_ANSWER_GATE_H_
#define FOGATA_ANSWER_GATE_H_

// Fogata: whether a method's answer may still be given from a thread of its own. The
// plugin answers some calls from threads of their own (the sound server's checks; on
// Windows a connection's addTransceiver and addTrack, which block for the first audio
// sender's set-up), through the task runner and channel that go with the plugin. When
// the plugin goes (the application quitting) it closes the gate, under its lock, and an
// answer that comes later is dropped instead of reaching a task runner already freed.
// An answer given under the lock is queued before the plugin can go: it waits on the
// lock meanwhile. Header only, with no libwebrtc, so that its test
// (test/answer_gate_test.cc) builds anywhere.

#include <mutex>
#include <utility>

namespace flutter_webrtc_plugin {

struct AnswerGate {
  std::mutex lock;
  bool open = true;
};

// Runs [answer] under the gate's lock, if the gate is still open; whether it ran.
template <typename Answer>
bool AnswerIfOpen(AnswerGate& gate, Answer&& answer) {
  std::lock_guard<std::mutex> guard(gate.lock);
  if (!gate.open) return false;
  std::forward<Answer>(answer)();
  return true;
}

// Closes the gate: no answer runs after this returns.
inline void CloseGate(AnswerGate& gate) {
  std::lock_guard<std::mutex> guard(gate.lock);
  gate.open = false;
}

}  // namespace flutter_webrtc_plugin

#endif  // FOGATA_ANSWER_GATE_H_
