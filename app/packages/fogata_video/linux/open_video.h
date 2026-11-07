#ifndef FOGATA_VIDEO_LINUX_OPEN_VIDEO_H_
#define FOGATA_VIDEO_LINUX_OPEN_VIDEO_H_

// Opening a video file once and checking its first bytes, apart from GTK and GStreamer so
// that test/open_video_test.cc builds it on its own.

#include <errno.h>
#include <fcntl.h>
#include <sys/stat.h>
#include <unistd.h>

#include <cstdint>
#include <string>
#include <vector>

#include "../common/video_head.h"

namespace fogata_video {

// A video file opened once: its first bytes were checked, and GStreamer reads it through
// /proc/self/fd, which opens the very file this descriptor has open (whatever its name
// leads to by then), never whatever is at its path later.
struct Opened {
  int fd = -1;
  std::string uri;
};

// Opens the regular file at `path`, read only, and checks that it begins as one of
// `heads`; false (and nothing kept open) otherwise. A pipe or a device is not opened for
// reading at all: opening it does not wait for a writer.
inline bool open_video(const char* path, const std::vector<Head>& heads,
                       Opened* out) {
  const int fd = open(path, O_RDONLY | O_CLOEXEC | O_NOCTTY | O_NONBLOCK);
  if (fd < 0) return false;
  struct stat st = {};
  uint8_t head[kHeadBytes];
  ssize_t got = -1;
  if (fstat(fd, &st) == 0 && S_ISREG(st.st_mode) &&
      fcntl(fd, F_SETFL, 0) == 0) {
    do {
      got = pread(fd, head, sizeof head, 0);
    } while (got < 0 && errno == EINTR);
  }
  if (got < 0 ||
      !HeadMatches(heads, head, static_cast<size_t>(got))) {
    close(fd);
    return false;
  }
  out->fd = fd;
  out->uri = "file:///proc/self/fd/" + std::to_string(fd);
  return true;
}

}  // namespace fogata_video

#endif  // FOGATA_VIDEO_LINUX_OPEN_VIDEO_H_
