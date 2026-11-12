#ifndef FOGATA_TURNS_H_
#define FOGATA_TURNS_H_

// Fogata: work that may wait (a sound server's answer) run off the asking thread, one
// piece after the other in the order asked. Header only, plain C++, so its tests build
// anywhere (common/cpp/test/silent_input_test.cc).

#include <functional>
#include <future>
#include <memory>
#include <thread>
#include <utility>

namespace flutter_webrtc_plugin {

class Turns {
 public:
  // Runs `turn` on a thread of its own once every turn asked before it is done (its
  // answer given too, so answers come in the order asked). Asked from one thread only;
  // the threads are not waited for.
  void Run(std::function<void()> turn) {
    std::shared_future<void> before = last_;
    auto done = std::make_shared<std::promise<void>>();
    last_ = done->get_future().share();
    std::thread([before, done, turn = std::move(turn)]() {
      if (before.valid()) before.wait();
      turn();
      done->set_value();
    }).detach();
  }

 private:
  std::shared_future<void> last_;
};

}  // namespace flutter_webrtc_plugin

#endif  // FOGATA_TURNS_H_
