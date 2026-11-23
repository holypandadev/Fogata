// The plugin waiting, as it goes, for the threads of their own that hold libwebrtc's
// objects (common/cpp/include/fogata_worker_fence.h), built with ThreadSanitizer by
// run_tests.sh: as the Windows plugin's addTransceiver and addTrack threads hold the
// connection while their blocking call runs, whatever the application did meanwhile.
// A library stands for libwebrtc, ended once the plugin's wait returns: no thread reads
// from it or lets go of what it made after it ended. Plain C++.

#include <atomic>
#include <chrono>
#include <condition_variable>
#include <cstdio>
#include <functional>
#include <memory>
#include <mutex>
#include <random>
#include <string>
#include <thread>
#include <vector>

#include "fogata_worker_fence.h"

using flutter_webrtc_plugin::WorkerFence;
using std::chrono::milliseconds;

namespace {

int failures = 0;

void check(bool ok, const std::string& what) {
  if (!ok) {
    std::printf("FAIL: %s\n", what.c_str());
    failures++;
  }
}

// Stands for libwebrtc: read and let go of only while it has not ended.
struct Library {
  std::atomic<bool> ended{false};
  std::atomic<int> late{0};      // reads and lets go after it ended
  std::atomic<int> released{0};  // objects let go of
  void Touch() {
    if (ended) late++;
  }
};

// A latch: Wait blocks until Let.
struct Latch {
  std::mutex lock;
  std::condition_variable wake;
  bool open = false;
  void Wait() {
    std::unique_lock<std::mutex> guard(lock);
    wake.wait(guard, [this] { return open; });
  }
  void Let() {
    std::lock_guard<std::mutex> guard(lock);
    open = true;
    wake.notify_all();
  }
};

// What a thread holds: the connection and what its call returns. Letting go of it may
// take a while (a connection's last hold closes it on the signalling thread).
struct Held {
  Held(Library* library, std::function<void()> release)
      : library(library), release(std::move(release)) {}
  ~Held() {
    if (release) release();
    library->Touch();
    library->released++;
  }
  Library* library;
  std::function<void()> release;
};

// One thread as the plugin runs it: in, its call, the answer read from the library, out.
void Worker(std::shared_ptr<WorkerFence> fence, std::shared_ptr<Library> library,
            std::function<void()> call, std::function<void()> release,
            std::atomic<int>* entered) {
  auto held = std::make_unique<Held>(library.get(), std::move(release));
  if (!fence->Enter()) {
    WorkerFence::Keep(std::move(held));
    return;
  }
  if (entered) (*entered)++;
  call();
  if (!fence->Returned()) {
    WorkerFence::Keep(std::move(held));
    return;
  }
  library->Touch();  // what the call returned, read for the answer
  fence->Leave(std::move(held));
}

void WaitFor(const std::atomic<int>& count, int value) {
  while (count < value) std::this_thread::yield();
}

}  // namespace

int main() {
  // Threads that finish within the wait: each let go of what it held before it returns.
  {
    auto fence = std::make_shared<WorkerFence>();
    auto library = std::make_shared<Library>();
    std::atomic<int> entered{0};
    std::vector<std::thread> threads;
    for (int i = 0; i < 8; i++) {
      threads.emplace_back(
          Worker, fence, library,
          [i] { std::this_thread::sleep_for(milliseconds(5 * i)); },
          std::function<void()>(), &entered);
    }
    WaitFor(entered, 8);
    const auto start = std::chrono::steady_clock::now();
    const bool all = fence->CloseAndWait(milliseconds(2000));
    const auto waited = std::chrono::steady_clock::now() - start;
    check(all, "the wait gave up on threads that finished in time");
    // The calls take 35 ms at most: the wait ends with the last of them, not at its limit.
    check(waited < milliseconds(1000), "the wait was not woken when the threads finished");
    check(library->released == 8, "a thread had not let go of what it held when the wait returned");
    library->ended = true;
    for (auto& thread : threads) thread.join();
    check(library->late == 0, "a thread touched the library after it ended");
    check(library->released == 8, "something was let go of twice");
  }

  // A thread still in its call at the limit (the audio service stalled): the wait
  // returns at the limit, and the thread, once its call returns into the ended library,
  // reads nothing of it and lets go of nothing.
  {
    auto fence = std::make_shared<WorkerFence>();
    auto library = std::make_shared<Library>();
    auto latch = std::make_shared<Latch>();
    std::atomic<int> entered{0};
    std::thread stuck(Worker, fence, library, [latch] { latch->Wait(); },
                      std::function<void()>(), &entered);
    WaitFor(entered, 1);
    const auto start = std::chrono::steady_clock::now();
    const bool all = fence->CloseAndWait(milliseconds(100));
    const auto waited = std::chrono::steady_clock::now() - start;
    library->ended = true;
    latch->Let();
    stuck.join();
    check(!all, "a thread still in its call was taken as finished");
    check(waited >= milliseconds(100) && waited < milliseconds(3000),
          "the wait did not end at its limit");
    check(library->late == 0, "a thread given up on touched the ended library");
    check(library->released == 0, "what a thread given up on held was let go of");
  }

  // A thread whose call returned before the limit, still letting go of what it holds
  // when the limit comes: waited for to its end, past the limit.
  {
    auto fence = std::make_shared<WorkerFence>();
    auto library = std::make_shared<Library>();
    auto latch = std::make_shared<Latch>();
    std::atomic<bool> releasing{false};
    std::thread slow(
        Worker, fence, library, [] {},
        [latch, &releasing] {
          releasing = true;
          latch->Wait();
        },
        nullptr);
    while (!releasing) std::this_thread::yield();
    std::atomic<bool> waiting{false};
    std::atomic<bool> returned{false};
    std::atomic<bool> all{false};
    std::thread plugin([&] {
      waiting = true;
      all = fence->CloseAndWait(milliseconds(20));
      returned = true;
    });
    while (!waiting) std::this_thread::yield();
    // Ten times the limit.
    std::this_thread::sleep_for(milliseconds(200));
    check(!returned, "the wait returned while a thread was letting go of what it held");
    latch->Let();
    plugin.join();
    library->ended = true;
    slow.join();
    check(all, "a thread whose call returned in time was given up on");
    check(library->released == 1 && library->late == 0,
          "a thread's letting go was not waited for");
  }

  // A thread that comes after the plugin started going makes no call and lets go of
  // nothing.
  {
    auto fence = std::make_shared<WorkerFence>();
    auto library = std::make_shared<Library>();
    check(fence->CloseAndWait(milliseconds(10)), "a wait with no thread gave up");
    library->ended = true;
    std::atomic<bool> called{false};
    std::thread late(Worker, fence, library, [&called] { called = true; },
                     std::function<void()>(), nullptr);
    late.join();
    check(!called, "a thread made its call after the plugin went");
    check(library->released == 0 && library->late == 0,
          "a thread after the plugin went touched the library");
  }

  // Threads finishing around the limit, at random: whatever each did, nothing reached
  // the library once it ended.
  std::mt19937 random(11);
  for (int round = 0; round < 100; round++) {
    auto fence = std::make_shared<WorkerFence>();
    auto library = std::make_shared<Library>();
    std::vector<std::thread> threads;
    for (int i = 0; i < 6; i++) {
      const int call_ms = static_cast<int>(random() % 30);
      const int release_ms = static_cast<int>(random() % 5);
      threads.emplace_back(
          Worker, fence, library,
          [call_ms] { std::this_thread::sleep_for(milliseconds(call_ms)); },
          [release_ms] { std::this_thread::sleep_for(milliseconds(release_ms)); },
          nullptr);
    }
    std::this_thread::sleep_for(milliseconds(random() % 10));
    fence->CloseAndWait(milliseconds(random() % 20));
    const int released = library->released;
    library->ended = true;
    for (auto& thread : threads) thread.join();
    check(library->late == 0 && library->released == released,
          "a thread touched the library after it ended (round " + std::to_string(round) +
              ")");
  }

  if (failures == 0) std::printf("worker fence: all passed\n");
  return failures == 0 ? 0 : 1;
}
