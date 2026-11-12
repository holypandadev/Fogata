// The "Fogata silence" input (linux/silent_input.cc, with fogata_turns.h), the plugin's own
// code against a PipeWire server played by the test (pipewire_fake.h): requests asked one
// after the other from the platform thread (fogataSilentInput) are carried out in that
// order on threads of their own, whatever the server's delays, and an answer comes for each
// while the plugin is there and none after it went. Built with ThreadSanitizer by
// run_tests.sh: any data race between those threads, PipeWire's and the platform thread
// fails the run.
//
// What it guards: a call ending while the silent input was being chosen (a share's sound
// with the microphone off) asks for it to be removed right after asking for it; were the
// removal to run first, WebRTC would record silence in every call after.

#include <chrono>
#include <condition_variable>
#include <cstdio>
#include <memory>
#include <mutex>
#include <string>
#include <thread>
#include <vector>

#include "pipewire_fake.h"
#include "silent_input.h"

using flutter_webrtc_plugin::SilentInput;

namespace {

int failures = 0;

void Check(bool ok, const std::string& what) {
  if (!ok) {
    std::printf("FAIL: %s\n", what.c_str());
    failures++;
  }
}

// The plugin's gate for answers (FlutterWebRTC::AnswerGate): closed when it goes.
struct Gate {
  std::mutex lock;
  bool open = true;
};

// Requests asked as fogataSilentInput asks them, and what came of each: the input there
// or not once it was carried out, and its answer.
class Requests {
 public:
  void Ask(bool on) {
    const int index = asked_++;
    {
      std::lock_guard<std::mutex> guard(lock_);
      there_.push_back(-1);
      answers_.push_back(-1);
    }
    std::shared_ptr<Gate> gate = gate_;
    SilentInput::Ask(on, [this, gate, index](bool made) {
      // What the server holds now: this request's work is done, the next has not
      // started (it waits for this one).
      const int there = pwfake::NodesAlive();
      std::lock_guard<std::mutex> guard(gate->lock);
      if (!gate->open) return;
      std::lock_guard<std::mutex> mine(lock_);
      there_[index] = there;
      answers_[index] = made ? 1 : 0;
      answered_++;
      changed_.notify_all();
    });
  }

  // Waits until `count` answers came, 20 s at most.
  bool Wait(int count) {
    std::unique_lock<std::mutex> guard(lock_);
    return changed_.wait_for(guard, std::chrono::seconds(20),
                             [&] { return answered_ >= count; });
  }

  // The plugin goes: no answer is given any more.
  int Close() {
    std::lock_guard<std::mutex> guard(gate_->lock);
    gate_->open = false;
    std::lock_guard<std::mutex> mine(lock_);
    return answered_;
  }

  int Answered() {
    std::lock_guard<std::mutex> guard(lock_);
    return answered_;
  }
  int There(int index) {
    std::lock_guard<std::mutex> guard(lock_);
    return there_[index];
  }
  int Answer(int index) {
    std::lock_guard<std::mutex> guard(lock_);
    return answers_[index];
  }

 private:
  std::shared_ptr<Gate> gate_ = std::make_shared<Gate>();
  int asked_ = 0;
  std::mutex lock_;
  std::condition_variable changed_;
  std::vector<int> there_;
  std::vector<int> answers_;
  int answered_ = 0;
};

// A request answered once all before it were, with the input there or not as asked.
void InOrder() {
  pwfake::SetAnswer(pwfake::Answer::kBound, 2);
  Requests requests;
  std::vector<bool> asked;
  unsigned seed = 12345;
  for (int i = 0; i < 60; i++) {
    seed = seed * 1103515245 + 12345;
    asked.push_back(((seed >> 16) & 3) != 0);
  }
  asked.push_back(false);
  for (bool on : asked) requests.Ask(on);
  Check(requests.Wait(int(asked.size())), "every request answered");
  for (size_t i = 0; i < asked.size(); i++) {
    const std::string which = "request " + std::to_string(i);
    Check(requests.There(int(i)) == (asked[i] ? 1 : 0),
          which + ": the input " + (asked[i] ? "made" : "removed") + " in its turn");
    Check(requests.Answer(int(i)) == 1, which + ": answered as made");
  }
  Check(pwfake::NodesAlive() == 0, "no input left after the last removal");
}

// The call ends while the server takes its time to make the input: the removal asked
// right after waits for the making, and the input does not stay.
void SlowServer() {
  pwfake::SetAnswer(pwfake::Answer::kBound, 300);
  Requests requests;
  requests.Ask(true);
  requests.Ask(false);
  Check(requests.Wait(2), "both answered");
  Check(requests.There(0) == 1 && requests.There(1) == 0, "made, then removed");
  Check(pwfake::NodesAlive() == 0, "no input left with a slow server");
}

// A server without the null sink factory says so, and one that never answers is given up
// after 2 s: the input is not made, and nothing is left of it.
void Refused() {
  for (pwfake::Answer how : {pwfake::Answer::kError, pwfake::Answer::kNone}) {
    pwfake::SetAnswer(how);
    Requests requests;
    requests.Ask(true);
    Check(requests.Wait(1), "a refused input answered");
    Check(requests.Answer(0) == 0, "a refused input answered as not made");
    Check(pwfake::NodesAlive() == 0, "nothing left of a refused input");
    requests.Ask(false);
    Check(requests.Wait(2), "its removal answered");
  }
  pwfake::SetAnswer(pwfake::Answer::kBound);
}

// The server restarted (as after an update): its node went with the connection, and the
// next request makes it again on a new one.
void Restarted() {
  pwfake::SetAnswer(pwfake::Answer::kBound);
  Requests requests;
  requests.Ask(true);
  Check(requests.Wait(1) && pwfake::NodesAlive() == 1, "made");
  pw_core* old = pwfake::NewestCore();
  pwfake::Post(old, [old] { pwfake::LoseConnection(old); });
  pwfake::Drain(old);
  Check(pwfake::NodesAlive() == 0, "gone with the server");
  requests.Ask(true);
  Check(requests.Wait(2) && requests.Answer(1) == 1, "made again");
  Check(pwfake::NewestCore() != old, "on a new connection");
  Check(pwfake::NodesAlive() == 1, "there again");
  requests.Ask(false);
  Check(requests.Wait(3) && pwfake::NodesAlive() == 0, "removed");
}

// The plugin goes while requests are still being carried out: none answers after.
void PluginGone() {
  pwfake::SetAnswer(pwfake::Answer::kBound, 20);
  Requests requests;
  for (int i = 0; i < 10; i++) requests.Ask(i % 2 == 0);
  Check(requests.Wait(2), "the first answers");
  const int before = requests.Close();
  // The rest still run (they hold nothing of the plugin); the last removal ends them.
  std::this_thread::sleep_for(std::chrono::milliseconds(800));
  Check(requests.Answered() == before, "no answer once the plugin went");
  pwfake::SetAnswer(pwfake::Answer::kBound);
  Requests after;
  after.Ask(false);
  Check(after.Wait(1), "requests go on");
  Check(pwfake::NodesAlive() == 0, "no input left");
}

}  // namespace

int main() {
  InOrder();
  SlowServer();
  Refused();
  Restarted();
  PluginGone();
  if (failures == 0) std::printf("silent_input_test: ok\n");
  return failures == 0 ? 0 : 1;
}
