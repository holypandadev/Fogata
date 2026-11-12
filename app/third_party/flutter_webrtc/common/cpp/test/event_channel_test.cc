// An event channel of the plugin (EventChannelProxyImpl, common/cpp/src/flutter_common.cc),
// the plugin's own code with Flutter's own channel and codec: libwebrtc's signalling thread
// sends events (a connection's observer) while the platform thread has Dart listen, cancel
// and listen again, and runs what was posted to it. Events sent before Dart listens go out
// first, none goes out twice or out of order, and every one sent while Dart listens
// arrives. Built with ThreadSanitizer by run_tests.sh: any data race fails the run.

#include <atomic>
#include <chrono>
#include <cstdio>
#include <deque>
#include <memory>
#include <mutex>
#include <string>
#include <thread>
#include <vector>

#include "flutter_common.h"
#include "task_runner.h"

namespace {

int failures = 0;

void Check(bool ok, const std::string& what) {
  if (!ok) {
    std::printf("FAIL: %s\n", what.c_str());
    failures++;
  }
}

// The platform thread's queue, as the Linux task runner keeps it: tasks run later, in the
// order queued, with the queue let go.
class Runner : public TaskRunner {
 public:
  void EnqueueTask(TaskClosure task) override {
    std::lock_guard<std::mutex> guard(lock_);
    tasks_.push_back(std::move(task));
  }
  void RunAll() {
    while (true) {
      TaskClosure task;
      {
        std::lock_guard<std::mutex> guard(lock_);
        if (tasks_.empty()) return;
        task = std::move(tasks_.front());
        tasks_.pop_front();
      }
      task();
    }
  }

 private:
  std::mutex lock_;
  std::deque<TaskClosure> tasks_;
};

// The engine's side of the channel: Dart's calls to it (listen, cancel), and the events
// that reach Dart.
class Messenger : public flutter::BinaryMessenger {
 public:
  void Send(const std::string&, const uint8_t* message, size_t size,
            flutter::BinaryReply) const override {
    // A success envelope: 0, then the event.
    if (message == nullptr || size < 2 || message[0] != 0) return;
    std::unique_ptr<EncodableValue> event =
        flutter::StandardMessageCodec::GetInstance().DecodeMessage(message + 1, size - 1);
    if (event == nullptr || !std::holds_alternative<int32_t>(*event)) return;
    std::lock_guard<std::mutex> guard(lock_);
    received_.push_back(std::get<int32_t>(*event));
  }

  void SetMessageHandler(const std::string&,
                         flutter::BinaryMessageHandler handler) override {
    handler_ = std::move(handler);
  }

  // Dart calls `method` on the channel, as on the platform thread.
  void Call(const std::string& method) {
    if (!handler_) return;
    const flutter::MethodCall<EncodableValue> call(method, nullptr);
    std::unique_ptr<std::vector<uint8_t>> message =
        flutter::StandardMethodCodec::GetInstance().EncodeMethodCall(call);
    handler_(message->data(), message->size(), [](const uint8_t*, size_t) {});
  }

  std::vector<int> Received() const {
    std::lock_guard<std::mutex> guard(lock_);
    return received_;
  }

  bool HasHandler() const { return bool(handler_); }

 private:
  mutable std::mutex lock_;
  mutable std::vector<int> received_;
  flutter::BinaryMessageHandler handler_;
};

void Send(EventChannelProxy& channel, int event) {
  channel.Success(EncodableValue(int32_t(event)));
}

}  // namespace

int main() {
  Runner runner;
  Messenger messenger;
  std::unique_ptr<EventChannelProxy> channel =
      EventChannelProxy::Create(&messenger, &runner, "FlutterWebRTC/test");

  // Sent before Dart listens: kept, and sent first.
  for (int i = 0; i < 5; i++) Send(*channel, i);
  runner.RunAll();
  Check(messenger.Received().empty(), "nothing before Dart listens");
  messenger.Call("listen");
  runner.RunAll();
  Check(messenger.Received() == std::vector<int>({0, 1, 2, 3, 4}),
        "what came before Dart listened, in order");

  // libwebrtc's signalling thread sends all along; the platform thread has Dart cancel,
  // listen again, and listen while listening (a hot restart), and runs the tasks.
  std::atomic<int> next{5};
  std::atomic<bool> stop{false};
  std::thread signalling([&] {
    while (!stop) {
      const int event = next++;
      Send(*channel, event);
      // Now and then a pause, as a connection's events come: the platform thread keeps up.
      if (event % 32 == 0) std::this_thread::sleep_for(std::chrono::microseconds(20));
    }
  });
  for (int round = 0; round < 300; round++) {
    runner.RunAll();
    if (round % 3 == 0) {
      messenger.Call("cancel");
      runner.RunAll();
      messenger.Call("listen");
    }
    if (round % 7 == 0) messenger.Call("listen");
    std::this_thread::yield();
  }
  // Listening from here on: every event sent from now arrives (a margin for those under
  // way as Dart listened again).
  const int from = next.load() + 100;
  while (next.load() < from + 2000) runner.RunAll();
  stop = true;
  signalling.join();
  runner.RunAll();
  const int last = next.load() - 1;

  const std::vector<int> received = messenger.Received();
  bool increasing = true;
  for (size_t i = 1; i < received.size(); i++) {
    increasing = increasing && received[i] > received[i - 1];
  }
  Check(increasing, "no event twice nor out of order");
  size_t at = 0;
  while (at < received.size() && received[at] < from) at++;
  Check(received.size() - at == size_t(last - from + 1),
        "every event sent while Dart listened arrived: " +
            std::to_string(received.size() - at) + " of " +
            std::to_string(last - from + 1));

  // Dart stops listening: what is sent then waits for it.
  messenger.Call("cancel");
  const size_t before = messenger.Received().size();
  Send(*channel, last + 1);
  runner.RunAll();
  Check(messenger.Received().size() == before, "nothing while Dart does not listen");
  messenger.Call("listen");
  runner.RunAll();
  Check(messenger.Received().size() == before + 1 &&
            messenger.Received().back() == last + 1,
        "kept until Dart listened again");

  channel.reset();
  Check(!messenger.HasHandler(), "the channel lets the engine go when freed");
  if (failures == 0) std::printf("event_channel_test: ok\n");
  return failures == 0 ? 0 : 1;
}
