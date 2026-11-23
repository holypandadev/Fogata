#ifndef FOGATA_WORKER_FENCE_H_
#define FOGATA_WORKER_FENCE_H_

// Fogata: the plugin waits, as it goes, for the threads of their own that hold
// libwebrtc's objects. On Windows a connection's addTransceiver and addTrack run on such
// a thread (the first audio sender's set-up blocks for about 600 ms), holding the
// connection, the track and what the call returns. When the application quit while one
// was still in its call, the plugin went and libwebrtc ended (~FlutterWebRTCBase) under
// it, and the thread went on with those objects afterwards: it read what its call
// returned and let go of all of it, on libwebrtc already ended.
//
// A thread goes through three steps: Enter before its call, Returned once its call has
// returned, Leave once it is done with everything of libwebrtc's. The plugin, as it goes
// and before libwebrtc ends, refuses every later Enter and waits up to a limit for the
// threads in their call. A thread whose call returned in time finishes (reads what the
// call returned, answers, lets go of what it holds) and is waited for to its end: its
// call returned, so libwebrtc's signalling thread was answering. A thread that comes
// after the plugin started going makes no call. One still in its call at the limit is
// left in it, as nothing can take it out: libwebrtc ends under that call, as it did
// before; once the call returns, the thread reads nothing of it and keeps what it holds
// for good (the process is quitting, and ends 10 s after a quit begins in any case).
//
// The lock is never held while a thread calls libwebrtc or lets go of what it holds.
// Header only, with no libwebrtc, so that its test (test/worker_fence_test.cc) builds
// anywhere.

#include <chrono>
#include <condition_variable>
#include <memory>
#include <mutex>
#include <utility>

namespace flutter_webrtc_plugin {

class WorkerFence {
 public:
  // A thread about to make its call: false once the plugin started going, and then it
  // makes no call and keeps what it holds (Keep).
  bool Enter() {
    std::lock_guard<std::mutex> guard(lock_);
    if (closed_) return false;
    inside_++;
    return true;
  }

  // A thread whose call returned, after Enter: true if it may go on with libwebrtc
  // (read what its call returned, answer, then Leave, which lets go of what it holds);
  // false if the plugin stopped waiting for it, and then it has left already and keeps
  // what it holds (Keep), touching nothing of libwebrtc's.
  bool Returned() {
    std::lock_guard<std::mutex> guard(lock_);
    inside_--;
    // The plugin's wait is woken either way: it waits for inside_ to reach none.
    done_.notify_all();
    if (abandoned_) return false;
    finishing_++;
    return true;
  }

  // A thread done, after Returned answered true: lets go of [held] here.
  template <typename Held>
  void Leave(std::unique_ptr<Held> held) {
    held.reset();
    std::lock_guard<std::mutex> guard(lock_);
    finishing_--;
    done_.notify_all();
  }

  // Never lets go of [held]: what a thread holds once libwebrtc may have ended.
  template <typename Held>
  static void Keep(std::unique_ptr<Held> held) {
    (void)held.release();
  }

  // As the plugin goes, before libwebrtc ends: refuses every later Enter and waits up to
  // [limit] for the threads in their call. Then those still in it are given up on, and
  // those whose call returned are waited for to their end. Whether no thread was given
  // up on.
  bool CloseAndWait(std::chrono::milliseconds limit) {
    std::unique_lock<std::mutex> guard(lock_);
    closed_ = true;
    const bool in_time =
        done_.wait_for(guard, limit, [this] { return inside_ == 0; });
    abandoned_ = true;
    done_.wait(guard, [this] { return finishing_ == 0; });
    return in_time;
  }

 private:
  std::mutex lock_;
  std::condition_variable done_;
  bool closed_ = false;
  bool abandoned_ = false;
  int inside_ = 0;     // threads in their call
  int finishing_ = 0;  // threads whose call returned in time, not done yet
};

}  // namespace flutter_webrtc_plugin

#endif  // FOGATA_WORKER_FENCE_H_
