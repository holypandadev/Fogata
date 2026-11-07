#include "task_runner_linux.h"

#include <glib.h>

namespace flutter_webrtc_plugin {

namespace {

// Runs the tasks queued so far, each with the queue let go.
gboolean RunTasks(gpointer data) {
  const auto& queue = *static_cast<std::shared_ptr<TaskRunnerLinux::Queue>*>(data);
  while (true) {
    TaskClosure task;
    {
      std::lock_guard<std::mutex> lock(queue->mutex);
      if (!queue->open || queue->tasks.empty()) break;
      task = std::move(queue->tasks.front());
      queue->tasks.pop();
    }
    task();
  }
  return G_SOURCE_REMOVE;
}

void ForgetQueue(gpointer data) {
  delete static_cast<std::shared_ptr<TaskRunnerLinux::Queue>*>(data);
}

}  // namespace

TaskRunnerLinux::~TaskRunnerLinux() {
  std::lock_guard<std::mutex> lock(queue_->mutex);
  queue_->open = false;
  std::queue<TaskClosure>().swap(queue_->tasks);
}

void TaskRunnerLinux::EnqueueTask(TaskClosure task) {
  {
    std::lock_guard<std::mutex> lock(queue_->mutex);
    if (!queue_->open) return;
    queue_->tasks.push(std::move(task));
  }
  // Always later, from the main context's loop: g_main_context_invoke ran the
  // tasks at once when called on the platform thread, inside whatever queued them.
  GSource* source = g_idle_source_new();
  g_source_set_callback(source, RunTasks,
                        new std::shared_ptr<Queue>(queue_), ForgetQueue);
  g_source_attach(source, g_main_context_default());
  g_source_unref(source);
}

}  // namespace flutter_webrtc_plugin
