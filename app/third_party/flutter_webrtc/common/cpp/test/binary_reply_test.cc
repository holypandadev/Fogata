// The reply to a message from Dart (Linux, linux/flutter/binary_reply.h): sent once on the
// message's response handle, which is let go of right after, once; a second reply goes
// nowhere and lets go of nothing more; a message never replied to keeps its handle. Every
// method called on the plugin kept its handle for good. Plain C++, no GLib.

#include <cstdint>
#include <cstdio>
#include <string>

#include "binary_reply.h"

using flutter::ReplyOnce;

namespace {

int failures = 0;

void check(bool ok, const std::string& what) {
  if (!ok) {
    std::printf("FAIL: %s\n", what.c_str());
    failures++;
  }
}

// A response handle, with the references taken on it and what was sent on it.
struct Handle {
  int refs = 1;
  int replies = 0;
  size_t last_size = 0;
};

std::function<void(const uint8_t*, size_t)> ReplyOn(Handle* handle) {
  handle->refs++;
  return ReplyOnce(
      handle,
      [](Handle* on, const uint8_t*, size_t size) {
        check(on->refs > 0, "sent on a handle still held");
        on->replies++;
        on->last_size = size;
      },
      [](Handle* on) { on->refs--; });
}

void RepliedOnce() {
  Handle handle;
  auto reply = ReplyOn(&handle);
  handle.refs--;  // The engine lets go of its own once the message is handed over.
  check(handle.refs == 1, "held until the reply");
  const uint8_t data[3] = {1, 2, 3};
  reply(data, sizeof(data));
  check(handle.replies == 1 && handle.last_size == 3, "the reply goes out on the handle");
  check(handle.refs == 0, "let go of once the reply is sent: " + std::to_string(handle.refs));
  reply(data, sizeof(data));
  check(handle.replies == 1, "a second reply goes nowhere");
  check(handle.refs == 0, "and lets go of nothing more");
}

void NeverReplied() {
  Handle handle;
  {
    auto reply = ReplyOn(&handle);
    handle.refs--;
  }
  check(handle.replies == 0, "nothing sent");
  check(handle.refs == 1, "a handle never replied to stays held");
}

}  // namespace

int main() {
  RepliedOnce();
  NeverReplied();
  if (failures == 0) std::printf("binary_reply_test: ok\n");
  return failures == 0 ? 0 : 1;
}
