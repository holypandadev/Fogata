// Checks open_video.h, as the Linux player opens a video: once, a plain file only, its first
// bytes checked on what it opened, and nothing left open when it is refused:
//   c++ -std=c++17 -Wall -Wextra -Werror open_video_test.cc -o /tmp/t && /tmp/t <clip.mp4>
#include "../open_video.h"

#include <dirent.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/stat.h>

#include <chrono>
#include <cstring>
#include <string>
#include <vector>

namespace {

int failures = 0;

void Check(bool ok, const char* what) {
  std::printf("%s: %s\n", ok ? "ok  " : "FAIL", what);
  if (!ok) failures++;
}

// How many descriptors this process has open.
int OpenFiles() {
  int n = 0;
  DIR* dir = opendir("/proc/self/fd");
  if (dir == nullptr) return -1;
  while (readdir(dir) != nullptr) n++;
  closedir(dir);
  return n;
}

bool Copy(const std::string& from, const std::string& to) {
  FILE* in = fopen(from.c_str(), "rb");
  FILE* out = fopen(to.c_str(), "wb");
  if (in == nullptr || out == nullptr) return false;
  char buffer[65536];
  size_t n;
  while ((n = fread(buffer, 1, sizeof buffer, in)) > 0) fwrite(buffer, 1, n, out);
  fclose(in);
  fclose(out);
  return true;
}

}  // namespace

int main(int argc, char** argv) {
  using fogata_video::Head;
  using fogata_video::Opened;
  using fogata_video::open_video;
  if (argc < 2) {
    std::printf("usage: open_video_test <clip.mp4>\n");
    return 2;
  }
  char scratch[] = "/tmp/fogata-open-video-XXXXXX";
  if (mkdtemp(scratch) == nullptr) return 2;
  const std::string dir = scratch;
  const std::string path = dir + "/clip.mp4";
  Check(Copy(argv[1], path), "the clip is copied");
  const std::vector<Head> mp4 = {
      {{0, 0, 0, 0, 'f', 't', 'y', 'p'}, {0, 0, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF}}};
  const std::vector<Head> mkv = {{{0x1A, 0x45, 0xDF, 0xA3}, {0xFF, 0xFF, 0xFF, 0xFF}}};
  const int before = OpenFiles();
  Opened refused;
  Check(!open_video(path.c_str(), mkv, &refused), "a file not what its name says is refused");
  Check(!open_video(path.c_str(), {}, &refused), "and one with no heads");
  Check(!open_video(dir.c_str(), mp4, &refused), "and a folder");
  Check(!open_video((dir + "/none.mp4").c_str(), mp4, &refused), "and nothing at all");
  const std::string fifo = dir + "/pipe.mp4";
  mkfifo(fifo.c_str(), 0600);
  const auto start = std::chrono::steady_clock::now();
  Check(!open_video(fifo.c_str(), mp4, &refused), "and a pipe");
  Check(std::chrono::steady_clock::now() - start < std::chrono::seconds(1),
        "without waiting for a writer");
  Check(refused.fd == -1, "none of them gives a descriptor");
  Check(OpenFiles() == before, "nor leaves one open");

  Opened opened;
  Check(open_video(path.c_str(), mp4, &opened), "the clip is opened");
  Check(OpenFiles() == before + 1, "with one descriptor");
  Check(opened.uri == "file:///proc/self/fd/" + std::to_string(opened.fd),
        "read through /proc/self/fd");
  // Another file put in its place: what the descriptor's path opens is still the clip.
  const std::string other = dir + "/other.mp4";
  FILE* junk = fopen(other.c_str(), "wb");
  fputs("not a video at all, but a file put in the clip's place", junk);
  fclose(junk);
  rename(other.c_str(), path.c_str());
  const std::string through = "/proc/self/fd/" + std::to_string(opened.fd);
  FILE* again = fopen(through.c_str(), "rb");
  unsigned char head[8] = {};
  const size_t got = again == nullptr ? 0 : fread(head, 1, sizeof head, again);
  if (again != nullptr) fclose(again);
  Check(got == 8 && std::memcmp(head + 4, "ftyp", 4) == 0,
        "after the file at its path is swapped, the clip is still what is read");
  Check(!open_video(path.c_str(), mp4, &refused), "and what was put there is refused");
  close(opened.fd);
  Check(OpenFiles() == before, "closed, nothing is left open");
  unlink(path.c_str());
  unlink(fifo.c_str());
  rmdir(dir.c_str());
  std::printf(failures == 0 ? "all passed\n" : "%d failed\n", failures);
  return failures == 0 ? 0 : 1;
}
