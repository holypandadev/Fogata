// A shared screen's sound started off the platform thread (Windows,
// common/cpp/include/fogata_sound_start.h), built with ThreadSanitizer by run_tests.sh: the
// start is told on the platform thread exactly once, unless the platform thread cancelled
// it first; a capture that opens in time is handed over and never let go of by the start;
// one that fails, opens after the limit or after a cancel is let go of once, and never
// handed over; one opened when the plugin went (its queue closed, or its task dropped
// unrun) is let go of once by the closer; and many starts with cancels at any moment
// have no data race. The audio service is a fake whose opening the test ends when it
// wants (at once, later or never), and the platform thread is the test's own, running
// the tasks queued for it. Plain C++, no Windows.

#include <atomic>
#include <chrono>
#include <condition_variable>
#include <cstdio>
#include <deque>
#include <functional>
#include <memory>
#include <mutex>
#include <random>
#include <string>
#include <thread>
#include <utility>
#include <vector>

#include "fogata_sound_start.h"

using flutter_webrtc_plugin::PlatformQueue;
using flutter_webrtc_plugin::SoundOpening;
using flutter_webrtc_plugin::SoundOutcome;
using flutter_webrtc_plugin::SoundStart;
using std::chrono::milliseconds;

namespace {

int failures = 0;

void check(bool ok, const std::string& what) {
  if (!ok) {
    std::printf("FAIL: %s\n", what.c_str());
    failures++;
  }
}

// Polls |done| until it holds, 10 s at most.
bool Eventually(const std::function<bool()>& done) {
  const auto end = std::chrono::steady_clock::now() + std::chrono::seconds(10);
  while (!done()) {
    if (std::chrono::steady_clock::now() > end) return false;
    std::this_thread::sleep_for(milliseconds(1));
  }
  return true;
}

// The audio service: its opening ends at once, when the test releases it, or after a
// pause, with the result given.
class FakeOpening : public SoundOpening {
 public:
  enum class Mode { kAtOnce, kWhenReleased, kAfterPause };

  FakeOpening(Mode mode, bool result, int pause_us = 0)
      : mode_(mode), result_(result), pause_us_(pause_us) {}

  bool Open() override {
    if (mode_ == Mode::kAfterPause) {
      std::this_thread::sleep_for(std::chrono::microseconds(pause_us_));
    }
    std::unique_lock<std::mutex> guard(lock_);
    opens_++;
    if (mode_ == Mode::kWhenReleased) {
      released_changed_.wait(guard, [this]() { return released_; });
    }
    returned_ = true;
    return result_;
  }

  void Close() override {
    std::lock_guard<std::mutex> guard(lock_);
    closes_++;
  }

  // Ends an opening that waits for the test, with |result|.
  void Release(bool result) {
    {
      std::lock_guard<std::mutex> guard(lock_);
      result_ = result;
      released_ = true;
    }
    released_changed_.notify_all();
  }

  int opens() {
    std::lock_guard<std::mutex> guard(lock_);
    return opens_;
  }

  int closes() {
    std::lock_guard<std::mutex> guard(lock_);
    return closes_;
  }

  // Whether Open returned, and with what.
  bool returned() {
    std::lock_guard<std::mutex> guard(lock_);
    return returned_;
  }

  bool result() {
    std::lock_guard<std::mutex> guard(lock_);
    return result_;
  }

 private:
  const Mode mode_;
  std::mutex lock_;
  std::condition_variable released_changed_;
  bool result_;
  const int pause_us_;
  bool released_ = false;
  bool returned_ = false;
  int opens_ = 0;
  int closes_ = 0;
};

// The platform thread's queue: this test's main thread runs what is queued, when it
// wants, or drops it unrun, as the plugin's runner does when it goes.
class FakePlatform {
 public:
  FakePlatform()
      : queue(std::make_shared<PlatformQueue>([this](std::function<void()> task) {
          std::lock_guard<std::mutex> guard(lock_);
          tasks_.push_back(std::move(task));
        })) {}

  // Nothing is queued here once this goes.
  ~FakePlatform() { queue->Close(); }

  SoundStart::Post Post() {
    std::shared_ptr<PlatformQueue> q = queue;
    return [q](std::function<void()> task) { return q->Post(std::move(task)); };
  }

  size_t Queued() {
    std::lock_guard<std::mutex> guard(lock_);
    return tasks_.size();
  }

  // Runs what is queued, outside the lock, until |done| holds (10 s at most).
  bool RunUntil(const std::function<bool()>& done) {
    const auto end = std::chrono::steady_clock::now() + std::chrono::seconds(10);
    for (;;) {
      RunQueued();
      if (done()) return true;
      if (std::chrono::steady_clock::now() > end) return false;
      std::this_thread::sleep_for(milliseconds(1));
    }
  }

  void RunQueued() {
    for (;;) {
      std::function<void()> task;
      {
        std::lock_guard<std::mutex> guard(lock_);
        if (tasks_.empty()) return;
        task = std::move(tasks_.front());
        tasks_.pop_front();
      }
      task();
    }
  }

  // Drops what is queued without running it.
  void DropAll() {
    std::deque<std::function<void()>> dropped;
    {
      std::lock_guard<std::mutex> guard(lock_);
      dropped.swap(tasks_);
    }
  }

  std::shared_ptr<PlatformQueue> queue;

 private:
  std::mutex lock_;
  std::deque<std::function<void()>> tasks_;
};

// What the closer was given, from whichever thread.
struct Closed {
  std::atomic<int> count{0};
};

SoundStart::Closer CloserFor(std::shared_ptr<Closed> closed) {
  return [closed](std::shared_ptr<SoundOpening> opening) {
    closed->count++;
    opening->Close();
  };
}

// What the platform thread was told; only touched there.
struct Told {
  std::vector<SoundOutcome> outcomes;
};

SoundStart::Settled TellerFor(Told* told) {
  return [told](SoundOutcome outcome) { told->outcomes.push_back(outcome); };
}

// Until the start is gone: its threads ended and its queued tasks ran or were dropped.
bool Gone(FakePlatform& platform, const std::weak_ptr<SoundStart>& start) {
  return platform.RunUntil([&]() { return start.expired(); });
}

void OpensInTime() {
  FakePlatform platform;
  auto opening = std::make_shared<FakeOpening>(FakeOpening::Mode::kAtOnce, true);
  auto closed = std::make_shared<Closed>();
  Told told;
  auto start = SoundStart::Begin(opening, std::chrono::seconds(10), platform.Post(),
                                 CloserFor(closed), TellerFor(&told));
  check(platform.RunUntil([&]() { return told.outcomes.size() == 1; }),
        "in time: told");
  check(told.outcomes.size() == 1 && told.outcomes[0] == SoundOutcome::kOpened,
        "in time: opened");
  std::shared_ptr<SoundOpening> taken = start->TakeOpened();
  check(taken == opening, "in time: handed over");
  check(start->TakeOpened() == nullptr, "in time: handed over once");
  check(!start->Cancel(), "in time: nothing to cancel once told");
  std::weak_ptr<SoundStart> weak = start;
  start.reset();
  check(Gone(platform, weak), "in time: the start goes");
  check(told.outcomes.size() == 1, "in time: told once");
  check(opening->opens() == 1, "in time: opened once");
  check(opening->closes() == 0 && closed->count == 0,
        "in time: what was handed over is not let go of by the start");
}

void FailsInTime() {
  FakePlatform platform;
  auto opening = std::make_shared<FakeOpening>(FakeOpening::Mode::kAtOnce, false);
  auto closed = std::make_shared<Closed>();
  Told told;
  auto start = SoundStart::Begin(opening, std::chrono::seconds(10), platform.Post(),
                                 CloserFor(closed), TellerFor(&told));
  check(platform.RunUntil([&]() { return told.outcomes.size() == 1; }),
        "failed: told");
  check(told.outcomes.size() == 1 && told.outcomes[0] == SoundOutcome::kFailed,
        "failed: as failed");
  check(start->TakeOpened() == nullptr, "failed: nothing handed over");
  std::weak_ptr<SoundStart> weak = start;
  start.reset();
  check(Gone(platform, weak), "failed: the start goes");
  check(opening->closes() == 1, "failed: let go of once, on its own thread");
  check(closed->count == 0, "failed: not by the closer");
  check(told.outcomes.size() == 1, "failed: told once");
}

// The audio service answers after the limit: the answer goes without the sound, and
// what opens late is let go of, never handed over.
void OpensTooLate() {
  FakePlatform platform;
  auto opening =
      std::make_shared<FakeOpening>(FakeOpening::Mode::kWhenReleased, true);
  auto closed = std::make_shared<Closed>();
  Told told;
  auto start = SoundStart::Begin(opening, milliseconds(20), platform.Post(),
                                 CloserFor(closed), TellerFor(&told));
  check(platform.RunUntil([&]() { return told.outcomes.size() == 1; }),
        "too late: told at the limit");
  check(told.outcomes.size() == 1 && told.outcomes[0] == SoundOutcome::kTimedOut,
        "too late: as timed out");
  check(!opening->returned(), "too late: told while still opening");
  check(start->TakeOpened() == nullptr, "too late: nothing handed over");
  check(!start->Cancel(), "too late: nothing to cancel once told");
  opening->Release(true);
  check(Eventually([&]() { return opening->closes() == 1; }),
        "too late: what opened late is let go of");
  check(start->TakeOpened() == nullptr, "too late: still nothing handed over");
  std::weak_ptr<SoundStart> weak = start;
  start.reset();
  check(Gone(platform, weak), "too late: the start goes");
  check(told.outcomes.size() == 1, "too late: told once");
  check(opening->closes() == 1 && closed->count == 0, "too late: let go of once");
}

// Another share with sound, or the capture's end, while the sound still opens.
void CancelledWhileOpening() {
  FakePlatform platform;
  auto opening =
      std::make_shared<FakeOpening>(FakeOpening::Mode::kWhenReleased, true);
  auto closed = std::make_shared<Closed>();
  Told told;
  auto start = SoundStart::Begin(opening, std::chrono::seconds(10), platform.Post(),
                                 CloserFor(closed), TellerFor(&told));
  check(start->Cancel(), "cancelled: the caller answers");
  check(!start->Cancel(), "cancelled: once");
  check(start->outcome() == SoundOutcome::kCancelled, "cancelled: as cancelled");
  opening->Release(true);
  check(Eventually([&]() { return opening->closes() == 1; }),
        "cancelled: what opens after is let go of");
  std::weak_ptr<SoundStart> weak = start;
  start.reset();
  check(Gone(platform, weak), "cancelled: the start goes");
  check(told.outcomes.empty(), "cancelled: never told");
  check(opening->closes() == 1 && closed->count == 0, "cancelled: let go of once");
}

// Opened in time, its answer queued but not yet run, when the platform thread cancels.
void CancelledOnceOpened() {
  FakePlatform platform;
  auto opening = std::make_shared<FakeOpening>(FakeOpening::Mode::kAtOnce, true);
  auto closed = std::make_shared<Closed>();
  Told told;
  auto start = SoundStart::Begin(opening, std::chrono::seconds(10), platform.Post(),
                                 CloserFor(closed), TellerFor(&told));
  check(Eventually([&]() { return platform.Queued() == 1; }),
        "cancelled once opened: its answer queued");
  check(start->Cancel(), "cancelled once opened: the caller answers");
  check(closed->count == 1 && opening->closes() == 1,
        "cancelled once opened: the closer lets go of it");
  check(start->TakeOpened() == nullptr, "cancelled once opened: nothing handed over");
  std::weak_ptr<SoundStart> weak = start;
  start.reset();
  check(Gone(platform, weak), "cancelled once opened: the start goes");
  check(told.outcomes.empty(), "cancelled once opened: the queued answer tells nothing");
  check(closed->count == 1 && opening->closes() == 1,
        "cancelled once opened: let go of once");
}

// Timed out, its answer queued, and cancelled before it ran.
void CancelledOnceTimedOut() {
  FakePlatform platform;
  auto opening =
      std::make_shared<FakeOpening>(FakeOpening::Mode::kWhenReleased, true);
  auto closed = std::make_shared<Closed>();
  Told told;
  auto start = SoundStart::Begin(opening, milliseconds(10), platform.Post(),
                                 CloserFor(closed), TellerFor(&told));
  check(Eventually([&]() { return platform.Queued() == 1; }),
        "cancelled once timed out: its answer queued");
  check(start->Cancel(), "cancelled once timed out: the caller answers");
  platform.RunQueued();
  check(told.outcomes.empty(), "cancelled once timed out: never told");
  opening->Release(true);
  check(Eventually([&]() { return opening->closes() == 1; }),
        "cancelled once timed out: what opens after is let go of");
  std::weak_ptr<SoundStart> weak = start;
  start.reset();
  check(Gone(platform, weak), "cancelled once timed out: the start goes");
  check(opening->closes() == 1 && closed->count == 0,
        "cancelled once timed out: let go of once");
}

// The plugin goes while the sound opens (its queue closes): the opening, once done,
// can tell nobody, and what opened is let go of when the start goes.
void PluginGoneWhileOpening() {
  FakePlatform platform;
  auto opening =
      std::make_shared<FakeOpening>(FakeOpening::Mode::kWhenReleased, true);
  auto closed = std::make_shared<Closed>();
  Told told;
  auto start = SoundStart::Begin(opening, std::chrono::seconds(10), platform.Post(),
                                 CloserFor(closed), TellerFor(&told));
  std::weak_ptr<SoundStart> weak = start;
  // As the plugin's destructor does, without cancelling: the worst case.
  platform.queue->Close();
  start.reset();
  opening->Release(true);
  // The closer runs as the start goes, after its last reference.
  check(Eventually([&]() { return closed->count == 1; }),
        "plugin gone: what opened is let go of, by the closer");
  check(Eventually([&]() { return weak.expired(); }), "plugin gone: the start goes");
  check(closed->count == 1 && opening->closes() == 1,
        "plugin gone: let go of once");
  check(platform.Queued() == 0, "plugin gone: nothing queued after the queue closed");
  check(told.outcomes.empty(), "plugin gone: never told");
}

// The plugin's runner goes with the answer still queued: the task is dropped unrun.
void TaskDroppedUnrun() {
  FakePlatform platform;
  auto opening = std::make_shared<FakeOpening>(FakeOpening::Mode::kAtOnce, true);
  auto closed = std::make_shared<Closed>();
  Told told;
  auto start = SoundStart::Begin(opening, std::chrono::seconds(10), platform.Post(),
                                 CloserFor(closed), TellerFor(&told));
  check(Eventually([&]() { return platform.Queued() == 1; }),
        "dropped: its answer queued");
  std::weak_ptr<SoundStart> weak = start;
  start.reset();
  platform.queue->Close();
  platform.DropAll();
  check(Eventually([&]() { return closed->count == 1; }),
        "dropped: what opened is let go of, by the closer");
  check(Eventually([&]() { return weak.expired(); }), "dropped: the start goes");
  check(closed->count == 1 && opening->closes() == 1, "dropped: let go of once");
  check(told.outcomes.empty(), "dropped: never told");
}

// Many starts at once, opening at any speed with any result, given any limit, and
// cancelled by the platform thread at any moment: each is answered once (told, or
// cancelled first), each capture that opened is either handed over or let go of once,
// and ThreadSanitizer sees no data race.
void ManyAtOnce() {
  const int kStarts = 200;
  FakePlatform platform;
  auto closed = std::make_shared<Closed>();
  std::mt19937 random(12345);
  std::vector<std::shared_ptr<FakeOpening>> openings;
  std::vector<std::shared_ptr<SoundStart>> starts;
  std::vector<std::weak_ptr<SoundStart>> weak(kStarts);
  std::vector<int> told(kStarts, 0);
  std::vector<int> cancelled(kStarts, 0);
  std::vector<std::shared_ptr<SoundOpening>> taken(kStarts);
  for (int i = 0; i < kStarts; i++) {
    const int pause = static_cast<int>(random() % 4000);
    const bool result = random() % 4 != 0;
    const int limit = static_cast<int>(random() % 5);
    auto opening = std::make_shared<FakeOpening>(FakeOpening::Mode::kAfterPause,
                                                 result, pause);
    openings.push_back(opening);
    // Settled runs only on this thread, from a task that holds the start.
    auto start = SoundStart::Begin(
        opening, milliseconds(limit), platform.Post(), CloserFor(closed),
        [i, &weak, &told, &taken](SoundOutcome outcome) {
          told[i]++;
          std::shared_ptr<SoundStart> self = weak[i].lock();
          if (outcome == SoundOutcome::kOpened && self) taken[i] = self->TakeOpened();
        });
    weak[i] = start;
    starts.push_back(start);
  }
  auto answered = [&]() {
    for (int i = 0; i < kStarts; i++) {
      if (told[i] + cancelled[i] == 0) return false;
    }
    return true;
  };
  const auto end = std::chrono::steady_clock::now() + std::chrono::seconds(10);
  while (!answered() && std::chrono::steady_clock::now() < end) {
    platform.RunQueued();
    const int i = static_cast<int>(random() % kStarts);
    if (random() % 3 == 0 && starts[i] && starts[i]->Cancel()) cancelled[i]++;
    std::this_thread::sleep_for(std::chrono::microseconds(200));
  }
  check(answered(), "many: every start answered");
  starts.clear();
  bool gone = platform.RunUntil([&]() {
    for (const auto& w : weak) {
      if (!w.expired()) return false;
    }
    return true;
  });
  check(gone, "many: every start goes");
  // A capture given up on is let go of by its own thread just before it lets go of the
  // start: waited for, not assumed.
  check(Eventually([&]() {
          for (int i = 0; i < kStarts; i++) {
            if (!taken[i] && openings[i]->closes() == 0) return false;
          }
          return true;
        }),
        "many: every capture not handed over let go of");
  int handed = 0;
  for (int i = 0; i < kStarts; i++) {
    const std::string which = "many: start " + std::to_string(i);
    check(told[i] + cancelled[i] == 1, which + " answered once");
    check(openings[i]->opens() == 1, which + " opened once");
    if (taken[i]) {
      handed++;
      check(told[i] == 1 && openings[i]->result(), which + " handed over only opened");
      check(openings[i]->closes() == 0, which + " handed over, not let go of");
    } else {
      check(openings[i]->closes() == 1, which + " let go of once");
    }
  }
  std::printf("sound_start_test: %d of %d handed over, %d cancelled first\n", handed,
              kStarts, [&]() {
                int n = 0;
                for (int c : cancelled) n += c;
                return n;
              }());
}

}  // namespace

int main() {
  OpensInTime();
  FailsInTime();
  OpensTooLate();
  CancelledWhileOpening();
  CancelledOnceOpened();
  CancelledOnceTimedOut();
  PluginGoneWhileOpening();
  TaskDroppedUnrun();
  ManyAtOnce();
  if (failures == 0) std::printf("sound_start_test: ok\n");
  return failures == 0 ? 0 : 1;
}
