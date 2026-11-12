#ifndef FOGATA_SOUND_START_H_
#define FOGATA_SOUND_START_H_

// Fogata: a shared screen's sound started off the platform thread (Windows). Opening the
// capture asks the system's audio service (an activation it answers on a thread of its
// own, then the client's set-up), which takes milliseconds when it is well and seconds,
// or never, when it is stalled: done on the platform thread, as the package did, it froze
// the window as a share with sound started. getDisplayMedia instead answers once the
// sound has opened, could not, or has taken too long, from the platform thread, and the
// platform thread itself never waits.
//
// A start ends in exactly one of these ways, told once on the platform thread (through
// the posted task) unless Cancel came first there:
//   kOpened    the capture opened in time; the platform thread takes it (TakeOpened).
//   kFailed    it could not open; it was let go of on the start's own thread.
//   kTimedOut  it was still opening at the limit; whatever opens later is let go of on
//              the start's own thread, never handed over.
//   kCancelled Cancel came first (another share with sound, the capture's end, the
//              plugin going); the caller answers itself, and nothing is told later.
// An opened capture that nobody takes (the plugin went before its task ran) is let go of
// by the closer when the start itself goes. Closing may wait on the audio service too,
// so it never runs on the platform thread: the start's own thread or the closer's.
//
// The lock is never held while calling out (Open, Close, the closer, posting, Settled).
// Header only, with no libwebrtc and no Windows, so its test
// (test/sound_start_test.cc) builds and runs under ThreadSanitizer on Linux.

#include <chrono>
#include <condition_variable>
#include <functional>
#include <memory>
#include <mutex>
#include <thread>
#include <utility>

namespace flutter_webrtc_plugin {

// The part of a sound's start that waits on the system's audio service.
class SoundOpening {
 public:
  virtual ~SoundOpening() = default;
  // Opens the capture without feeding anything yet. May wait seconds; called once, on
  // a thread of the start's own.
  virtual bool Open() = 0;
  // Lets go of what Open opened (nothing after a failed Open). May wait too; never
  // called on the platform thread, and never at the same time as Open.
  virtual void Close() = 0;
};

// Queues tasks for the platform thread while its owner is there. Close(), called on the
// platform thread as the owner goes and before its task runner does, makes every later
// Post refuse: a thread that outlives the owner (an activation the audio service never
// answers) then never reaches the runner. Posting holds the lock while queuing, so a
// Post either queued before Close or refuses.
class PlatformQueue {
 public:
  using Enqueue = std::function<void(std::function<void()>)>;

  explicit PlatformQueue(Enqueue enqueue) : enqueue_(std::move(enqueue)) {}

  bool Post(std::function<void()> task) {
    std::lock_guard<std::mutex> guard(lock_);
    if (!open_ || !enqueue_) return false;
    enqueue_(std::move(task));
    return true;
  }

  void Close() {
    std::lock_guard<std::mutex> guard(lock_);
    open_ = false;
  }

 private:
  std::mutex lock_;
  bool open_ = true;
  const Enqueue enqueue_;
};

enum class SoundOutcome { kOpening, kOpened, kFailed, kTimedOut, kCancelled };

class SoundStart : public std::enable_shared_from_this<SoundStart> {
 public:
  // Queues a task for the platform thread; false when it can no longer run there.
  using Post = std::function<bool(std::function<void()>)>;
  // Lets go of an opened capture off the platform thread (Close may wait).
  using Closer = std::function<void(std::shared_ptr<SoundOpening>)>;
  // How the start ended, on the platform thread, once; never after Cancel returned true.
  using Settled = std::function<void(SoundOutcome)>;

  // Opens |opening| on a thread of its own, and gives up on it after |limit|.
  static std::shared_ptr<SoundStart> Begin(std::shared_ptr<SoundOpening> opening,
                                           std::chrono::milliseconds limit,
                                           Post post,
                                           Closer closer,
                                           Settled settled) {
    std::shared_ptr<SoundStart> start(new SoundStart(
        opening, std::move(post), std::move(closer), std::move(settled)));
    const auto deadline = std::chrono::steady_clock::now() + limit;
    // Each thread holds the start, which holds nothing of the plugin's but what
    // |post| and |settled| capture: they run only on the platform thread while the
    // queue is open.
    std::thread([start, opening]() { start->Opened(opening->Open()); }).detach();
    std::thread([start, deadline]() { start->Watch(deadline); }).detach();
    return start;
  }

  // Platform thread. True when the start had not been told yet: it never will be, and
  // the caller answers itself. A capture still opening is let go of by its own thread
  // once it opens; one opened but not yet handed over goes to the closer now.
  bool Cancel() {
    std::shared_ptr<SoundOpening> to_close;
    {
      std::lock_guard<std::mutex> guard(lock_);
      if (told_) return false;
      told_ = true;
      if (outcome_ == SoundOutcome::kOpening) {
        outcome_ = SoundOutcome::kCancelled;
      } else if (outcome_ == SoundOutcome::kOpened) {
        to_close = std::move(opening_);
      }
    }
    changed_.notify_all();
    if (to_close) closer_(std::move(to_close));
    return true;
  }

  // Platform thread, from Settled with kOpened: the opened capture, now the caller's;
  // null otherwise or once taken.
  std::shared_ptr<SoundOpening> TakeOpened() {
    std::lock_guard<std::mutex> guard(lock_);
    if (outcome_ != SoundOutcome::kOpened) return nullptr;
    return std::move(opening_);
  }

  SoundOutcome outcome() {
    std::lock_guard<std::mutex> guard(lock_);
    return outcome_;
  }

  ~SoundStart() {
    // Opened, told, and never taken: the plugin went before its task ran, or its task
    // was dropped unrun with the runner. Nothing else holds the start by now.
    if (opening_ && outcome_ == SoundOutcome::kOpened) closer_(std::move(opening_));
  }

  SoundStart(const SoundStart&) = delete;
  SoundStart& operator=(const SoundStart&) = delete;

 private:
  SoundStart(std::shared_ptr<SoundOpening> opening,
             Post post,
             Closer closer,
             Settled settled)
      : opening_(std::move(opening)),
        post_(std::move(post)),
        closer_(std::move(closer)),
        settled_(std::move(settled)) {}

  // The start's own thread, once Open returned.
  void Opened(bool ok) {
    std::shared_ptr<SoundOpening> to_close;
    bool tell = false;
    {
      std::lock_guard<std::mutex> guard(lock_);
      if (outcome_ == SoundOutcome::kOpening) {
        outcome_ = ok ? SoundOutcome::kOpened : SoundOutcome::kFailed;
        tell = true;
        if (!ok) to_close = std::move(opening_);
      } else {
        // Timed out or cancelled meanwhile: what opened late is let go of here, and
        // never handed over (the answer went without it).
        to_close = std::move(opening_);
      }
    }
    changed_.notify_all();
    if (to_close) to_close->Close();
    if (tell) Tell();
  }

  // The watching thread: gives up at |deadline| on a start still opening.
  void Watch(std::chrono::steady_clock::time_point deadline) {
    bool tell = false;
    {
      std::unique_lock<std::mutex> guard(lock_);
      if (!changed_.wait_until(guard, deadline, [this]() {
            return outcome_ != SoundOutcome::kOpening;
          })) {
        outcome_ = SoundOutcome::kTimedOut;
        tell = true;
      }
    }
    if (tell) Tell();
  }

  // Off the platform thread, once the outcome is set: has it told there. Refused (the
  // plugin went), nothing is told, and an opened capture stays until the start goes.
  void Tell() {
    std::shared_ptr<SoundStart> self = shared_from_this();
    post_([self]() { self->Deliver(); });
  }

  // Platform thread.
  void Deliver() {
    SoundOutcome ended = SoundOutcome::kOpening;
    {
      std::lock_guard<std::mutex> guard(lock_);
      if (told_) return;  // Cancel came first.
      told_ = true;
      ended = outcome_;
    }
    settled_(ended);
  }

  std::mutex lock_;
  std::condition_variable changed_;
  SoundOutcome outcome_ = SoundOutcome::kOpening;
  // Whether the platform thread has its answer: Settled called, or Cancel returned true.
  bool told_ = false;
  // Held while opening, then until taken or let go of.
  std::shared_ptr<SoundOpening> opening_;
  const Post post_;
  const Closer closer_;
  const Settled settled_;
};

}  // namespace flutter_webrtc_plugin

#endif  // FOGATA_SOUND_START_H_
