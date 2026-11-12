#ifndef FLUTTER_WEBRTC_RETIRED_TEXTURES_H_
#define FLUTTER_WEBRTC_RETIRED_TEXTURES_H_

// Fogata: unregistered textures, kept until the engine is surely done with them (see
// core_implementations.cc). Header only, with neither GLib nor the engine, so that its
// test (common/cpp/test/retired_textures_test.cc) builds anywhere.

#include <cstdint>
#include <functional>
#include <mutex>
#include <utility>
#include <vector>

namespace flutter {

template <typename Texture>
class RetiredTextures {
 public:
  using Release = std::function<void()>;

  // On the platform thread: `texture` was unregistered, and nothing reads what it
  // showed through it any more (its pointer to it is already cleared); `release` frees
  // that. The serial to give to Expire later.
  uint64_t Retire(Texture texture, Release release) {
    std::lock_guard<std::mutex> lock(mutex_);
    retired_.push_back({texture, copies_, ++serials_, std::move(release)});
    return serials_;
  }

  // On the raster thread, as `current` is about to be copied: the textures it has moved
  // past, for their last unref there. Their releases wait for TakeReleased; `released`
  // says whether any does.
  std::vector<Texture> MovedOn(Texture current, bool* released) {
    std::vector<Texture> done;
    *released = false;
    std::lock_guard<std::mutex> lock(mutex_);
    copies_++;
    for (auto it = retired_.begin(); it != retired_.end();) {
      if (copies_ > it->after && it->texture != current) {
        done.push_back(it->texture);
        if (it->release) {
          released_.push_back(std::move(it->release));
          *released = true;
        }
        it = retired_.erase(it);
      } else {
        ++it;
      }
    }
    return done;
  }

  // On the platform thread: the releases MovedOn let go.
  std::vector<Release> TakeReleased() {
    std::vector<Release> run;
    std::lock_guard<std::mutex> lock(mutex_);
    run.swap(released_);
    return run;
  }

  // On the platform thread, long after `serial` was retired (seconds, where a copy takes
  // milliseconds): its release, if MovedOn has not let it go yet, or an empty one. Only a
  // copy that started before the texture was unregistered could still read what it
  // showed, and none takes that long; without this, what the last texture drawn showed
  // (a renderer and its picture, up to 132 MB at 8K) stayed until the engine copied
  // another texture, which in a call left with no picture may be never. The texture
  // itself stays for MovedOn: its last unref belongs on the raster thread.
  Release Expire(uint64_t serial) {
    std::lock_guard<std::mutex> lock(mutex_);
    for (auto& entry : retired_) {
      if (entry.serial == serial) {
        Release release = std::move(entry.release);
        // A moved-from function need not be empty.
        entry.release = nullptr;
        return release;
      }
    }
    return Release();
  }

 private:
  struct Entry {
    Texture texture;
    // How many copies had started when it was unregistered.
    uint64_t after;
    uint64_t serial;
    // Empty once run, or handed over to run.
    Release release;
  };

  std::mutex mutex_;
  std::vector<Entry> retired_;
  std::vector<Release> released_;
  // How many times the raster thread has started copying pixels.
  uint64_t copies_ = 0;
  uint64_t serials_ = 0;
};

}  // namespace flutter

#endif  // FLUTTER_WEBRTC_RETIRED_TEXTURES_H_
