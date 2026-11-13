#ifndef FLUTTER_WEBRTC_BINARY_REPLY_H_
#define FLUTTER_WEBRTC_BINARY_REPLY_H_

// Fogata: the one reply to a message from Dart, sent on the message's response handle
// (core_implementations.cc). The handle is held from the message until its reply has
// gone out, and let go of then: the package took a reference to it and never gave it
// back, so every method called on the plugin kept its handle, an object of the
// engine's, for good. A handle never replied to stays held, as before: the engine
// complains about one let go of unanswered, on whichever thread drops it. Header only,
// with neither GLib nor the engine, so that its test
// (common/cpp/test/binary_reply_test.cc) builds anywhere.

#include <cstddef>
#include <cstdint>
#include <functional>
#include <iostream>

namespace flutter {

// `held` is a reference to the handle, given over; `send(held, reply, size)` sends
// the reply on it, and `release(held)` gives the reference back once it has.
template <typename Handle, typename Send, typename Release>
std::function<void(const uint8_t*, size_t)> ReplyOnce(Handle held,
                                                      Send send,
                                                      Release release) {
  return [held, send, release](const uint8_t* reply,
                               size_t reply_size) mutable {
    if (!held) {
      std::cerr << "Error: Response can be set only once. Ignoring "
                   "duplicate response."
                << std::endl;
      return;
    }
    send(held, reply, reply_size);
    release(held);
    held = nullptr;
  };
}

}  // namespace flutter

#endif  // FLUTTER_WEBRTC_BINARY_REPLY_H_
