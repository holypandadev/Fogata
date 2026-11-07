#ifndef PACKAGES_FLUTTER_WEBRTC_LINUX_TASK_RUNNER_LINUX_H_
#define PACKAGES_FLUTTER_WEBRTC_LINUX_TASK_RUNNER_LINUX_H_

#include <memory>
#include <mutex>
#include <queue>
#include "task_runner.h"

namespace flutter_webrtc_plugin {

// Runs tasks on the GLib main context (the platform thread), queued from any
// thread. Fogata: each task is run later, from an idle source, never within
// EnqueueTask, and with the queue let go, so a task may queue another. The queue
// is shared with the sources still to run: once the runner goes they find it
// closed and run nothing.
class TaskRunnerLinux : public TaskRunner {
 public:
  TaskRunnerLinux() = default;
  ~TaskRunnerLinux() override;

  // TaskRunner implementation.
  void EnqueueTask(TaskClosure task) override;

  struct Queue {
    std::mutex mutex;
    std::queue<TaskClosure> tasks;
    bool open = true;
  };

 private:
  std::shared_ptr<Queue> queue_ = std::make_shared<Queue>();
};

}  // namespace flutter_webrtc_plugin

#endif  // PACKAGES_FLUTTER_WEBRTC_LINUX_TASK_RUNNER_LINUX_H_
