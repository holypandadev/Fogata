// Unregistered textures kept until the engine is done with them (Linux,
// linux/flutter/retired_textures.h), built with ThreadSanitizer by run_tests.sh: a texture
// goes once the raster thread copies another, never while it may still copy it; what it
// showed is let go of once, whether by that or by the timeout after its unregistering,
// so a call left with no picture does not keep the last one; and the platform thread
// retiring textures while a raster thread copies has no data race. Plain C++, no GLib.

#include <atomic>
#include <cstdio>
#include <string>
#include <thread>
#include <vector>

#include "retired_textures.h"

using flutter::RetiredTextures;

namespace {

int failures = 0;

void check(bool ok, const std::string& what) {
  if (!ok) {
    std::printf("FAIL: %s\n", what.c_str());
    failures++;
  }
}

void RunAll(std::vector<std::function<void()>> releases) {
  for (auto& release : releases) release();
}

// Textures are plain numbers here.
void MovedOnAnother() {
  RetiredTextures<int> retired;
  int freed = 0;
  retired.Retire(1, [&] { freed++; });
  bool released = false;
  check(retired.MovedOn(1, &released).empty(), "kept while it is the one copied");
  check(!released, "nothing to release yet");
  auto done = retired.MovedOn(2, &released);
  check(done.size() == 1 && done[0] == 1, "goes once another is copied");
  check(released, "its release waits");
  RunAll(retired.TakeReleased());
  check(freed == 1, "released once");
  check(retired.TakeReleased().empty(), "not twice");
}

void CopiedBeforeRetiring() {
  RetiredTextures<int> retired;
  bool released = false;
  // A copy of another texture that started before the unregistering proves nothing.
  retired.MovedOn(2, &released);
  int freed = 0;
  retired.Retire(1, [&] { freed++; });
  check(retired.MovedOn(2, &released).size() == 1, "the next copy of another frees it");
  RunAll(retired.TakeReleased());
  check(freed == 1, "released");
}

// No other texture is ever copied again: what it showed goes at the timeout, the
// texture itself stays for the raster thread, and nothing is released twice.
void Expired() {
  RetiredTextures<int> retired;
  int freed = 0;
  const uint64_t serial = retired.Retire(1, [&] { freed++; });
  auto release = retired.Expire(serial);
  check(bool(release), "released at the timeout");
  if (release) release();
  check(freed == 1, "freed at the timeout");
  check(!retired.Expire(serial), "not twice by the timeout");
  bool released = false;
  auto done = retired.MovedOn(2, &released);
  check(done.size() == 1 && done[0] == 1, "the texture still goes on the raster thread");
  check(!released, "with nothing more to release");
  RunAll(retired.TakeReleased());
  check(freed == 1, "freed once");
}

void ExpiredAfterMovingOn() {
  RetiredTextures<int> retired;
  int freed = 0;
  const uint64_t serial = retired.Retire(1, [&] { freed++; });
  bool released = false;
  retired.MovedOn(2, &released);
  RunAll(retired.TakeReleased());
  check(!retired.Expire(serial), "nothing left for the timeout");
  check(freed == 1, "freed once");
}

void Races() {
  RetiredTextures<int> retired;
  constexpr int kTextures = 5000;
  std::atomic<bool> stop{false};
  std::atomic<int> unrefs{0};
  std::thread raster([&] {
    int current = 0;
    while (!stop) {
      bool released = false;
      unrefs += int(retired.MovedOn(current, &released).size());
      current = (current + 1) % 3;
    }
  });
  int freed = 0;
  std::vector<uint64_t> serials;
  for (int i = 0; i < kTextures; i++) {
    serials.push_back(retired.Retire(100 + i, [&] { freed++; }));
    RunAll(retired.TakeReleased());
    if (i % 3 == 0 && i >= 10) {
      if (auto release = retired.Expire(serials[i - 10])) release();
    }
  }
  // The raster thread moves past every one of them.
  while (unrefs < kTextures) std::this_thread::yield();
  stop = true;
  raster.join();
  RunAll(retired.TakeReleased());
  check(freed == kTextures, "every one released once: " + std::to_string(freed));
  check(unrefs == kTextures, "every one let go of once");
}

}  // namespace

int main() {
  MovedOnAnother();
  CopiedBeforeRetiring();
  Expired();
  ExpiredAfterMovingOn();
  Races();
  if (failures == 0) std::printf("retired_textures_test: ok\n");
  return failures == 0 ? 0 : 1;
}
