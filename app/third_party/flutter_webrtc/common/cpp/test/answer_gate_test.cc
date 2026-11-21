// Answers given from threads of their own while the plugin goes
// (common/cpp/include/fogata_answer_gate.h), built with ThreadSanitizer by run_tests.sh: as
// the Windows plugin's addTransceiver and addTrack threads answer when their blocking call
// returns, whatever the application did meanwhile. Many threads answer into a queue that
// stands for the plugin's task runner while the plugin closes the gate and frees the
// queue: every answer that ran, ran before the close, none after, and none touched the
// queue once freed. Plain C++.

#include <atomic>
#include <chrono>
#include <cstdio>
#include <memory>
#include <random>
#include <string>
#include <thread>
#include <vector>

#include "fogata_answer_gate.h"

using flutter_webrtc_plugin::AnswerGate;
using flutter_webrtc_plugin::AnswerIfOpen;
using flutter_webrtc_plugin::CloseGate;

namespace {

int failures = 0;

void check(bool ok, const std::string& what) {
  if (!ok) {
    std::printf("FAIL: %s\n", what.c_str());
    failures++;
  }
}

// Stands for the plugin's task runner: freed as the plugin goes.
struct Queue {
  std::vector<int> tasks;
};

}  // namespace

int main() {
  std::mt19937 random(7);
  for (int round = 0; round < 200; round++) {
    auto gate = std::make_shared<AnswerGate>();
    auto* queue = new Queue();
    std::atomic<bool> closed{false};
    std::atomic<int> after{0};
    std::atomic<int> ran{0};
    std::vector<std::thread> threads;
    const int delay = static_cast<int>(random() % 200);
    for (int i = 0; i < 8; i++) {
      threads.emplace_back([gate, queue, i, delay, &closed, &after, &ran] {
        std::this_thread::sleep_for(std::chrono::microseconds(delay * i));
        AnswerIfOpen(*gate, [&] {
          if (closed) after++;
          queue->tasks.push_back(i);
          ran++;
        });
      });
    }
    std::this_thread::sleep_for(std::chrono::microseconds(random() % 800));
    // The plugin going: the gate closed, then what the answers would use freed.
    CloseGate(*gate);
    closed = true;
    const int before = ran;
    delete queue;
    for (auto& thread : threads) thread.join();
    check(after == 0, "an answer ran after the gate closed (round " +
                          std::to_string(round) + ")");
    check(ran == before, "an answer ran after the queue was freed (round " +
                             std::to_string(round) + ")");
  }
  // An open gate lets every answer through.
  AnswerGate gate;
  int count = 0;
  for (int i = 0; i < 3; i++) AnswerIfOpen(gate, [&] { count++; });
  check(count == 3, "an open gate held an answer back");
  CloseGate(gate);
  check(!AnswerIfOpen(gate, [&] { count++; }) && count == 3,
        "a closed gate let an answer through");
  if (failures == 0) std::printf("answer gate: all passed\n");
  return failures == 0 ? 0 : 1;
}
