// Reads videos' first pictures with video_probe.cpp, as the plugin does, and checks that a
// probe always ends in its time and lets go of what it opened:
//   probe_test.exe <clip.mp4> <scratch folder>
// The clip is the integration tests' (480 by 270, four seconds, its index before its data).
// A server on 127.0.0.1 in this program stands for a file that never comes (it answers
// nothing), and a byte stream of its own for a file that stops coming, while its index
// is read or once the first picture is asked for: its reads wait, as a decoder waits on
// a file that stalls.
// Built and run by run_tests.ps1.
#include <winsock2.h>
#include <ws2tcpip.h>

#include <windows.h>

#include <mfapi.h>
#include <mfidl.h>
#include <wrl/client.h>

#include <atomic>
#include <chrono>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <fstream>
#include <iterator>
#include <map>
#include <mutex>
#include <string>
#include <thread>
#include <vector>

#include "../video_probe.h"

using Clock = std::chrono::steady_clock;
using Microsoft::WRL::ComPtr;

namespace {

int failures = 0;

void Check(bool ok, const char* what) {
  std::printf("%s: %s\n", ok ? "ok  " : "FAIL", what);
  if (!ok) failures++;
}

std::wstring Wide(const std::string& text) {
  return std::wstring(text.begin(), text.end());
}

std::vector<char> ReadAll(const std::string& path) {
  std::ifstream in(path, std::ios::binary);
  return std::vector<char>(std::istreambuf_iterator<char>(in), {});
}

void WriteAll(const std::string& path, const char* data, size_t size) {
  std::ofstream out(path, std::ios::binary | std::ios::trunc);
  out.write(data, static_cast<std::streamsize>(size));
}

// What is put in a held clip's place: begins as an MP4 does, then nothing of one.
const char garbage_head[] = "\0\0\0\x20" "ftypisom" "nothing of a video here";

// A probe of `path` given `time`: its result and how long it took.
struct Timed {
  bool ok = false;
  fogata_video::Probe probe;
  double seconds = 0;
};

Timed Run(const std::wstring& path, std::chrono::milliseconds time) {
  Timed out;
  const auto start = Clock::now();
  out.ok = fogata_video::ProbeVideo(path, 320, &out.probe, time);
  out.seconds = std::chrono::duration<double>(Clock::now() - start).count();
  return out;
}

// Whether nothing holds the file open: it opens with no sharing at all (a rename would
// pass while Media Foundation still holds it, since it lets others delete).
bool Free(const std::string& path) {
  for (int i = 0; i < 40; i++) {
    HANDLE file = CreateFileA(path.c_str(), GENERIC_READ, 0, nullptr, OPEN_EXISTING,
                              FILE_ATTRIBUTE_NORMAL, nullptr);
    if (file != INVALID_HANDLE_VALUE) {
      CloseHandle(file);
      return true;
    }
    Sleep(50);
  }
  return false;
}

// Takes connections on 127.0.0.1, reads each request and answers nothing, until the
// other side closes. `closed` tells when it did.
class SilentServer {
 public:
  SilentServer() {
    listener_ = socket(AF_INET, SOCK_STREAM, IPPROTO_TCP);
    sockaddr_in at = {};
    at.sin_family = AF_INET;
    at.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
    at.sin_port = 0;
    bind(listener_, reinterpret_cast<sockaddr*>(&at), sizeof(at));
    int size = sizeof(at);
    getsockname(listener_, reinterpret_cast<sockaddr*>(&at), &size);
    port_ = ntohs(at.sin_port);
    listen(listener_, 4);
    thread_ = std::thread([this] { Serve(); });
  }
  ~SilentServer() {
    stop_ = true;
    closesocket(listener_);
    thread_.join();
  }
  int port() const { return port_; }
  bool closed() const { return closed_; }
  bool asked() const { return asked_; }

 private:
  void Serve() {
    // The client may connect more than once (a second request for a range): each
    // connection is held the same way.
    std::vector<std::thread> held;
    while (!stop_) {
      SOCKET client = accept(listener_, nullptr, nullptr);
      if (client == INVALID_SOCKET) break;
      held.emplace_back([this, client] { Hold(client); });
    }
    for (auto& t : held) t.join();
  }

  void Hold(SOCKET client) {
    char request[4096];
    recv(client, request, sizeof(request), 0);
    asked_ = true;
    // Nothing: wait for the client to go, or for the end of the test.
    DWORD wait = 250;
    setsockopt(client, SOL_SOCKET, SO_RCVTIMEO, reinterpret_cast<char*>(&wait),
               sizeof(wait));
    char byte;
    while (!stop_) {
      const int got = recv(client, &byte, 1, 0);
      if (got == 0 || (got < 0 && WSAGetLastError() != WSAETIMEDOUT)) {
        closed_ = true;
        break;
      }
    }
    closesocket(client);
  }

  SOCKET listener_ = INVALID_SOCKET;
  int port_ = 0;
  std::thread thread_;
  std::atomic<bool> stop_{false};
  std::atomic<bool> closed_{false};
  std::atomic<bool> asked_{false};
};

uint32_t Be32(const char* at) {
  const auto* b = reinterpret_cast<const unsigned char*>(at);
  return (uint32_t{b[0]} << 24) | (uint32_t{b[1]} << 16) | (uint32_t{b[2]} << 8) | b[3];
}

void PutBe32(char* at, uint32_t v) {
  at[0] = static_cast<char>(v >> 24);
  at[1] = static_cast<char>(v >> 16);
  at[2] = static_cast<char>(v >> 8);
  at[3] = static_cast<char>(v);
}

// Adds `delta` to every chunk offset (stco) in the boxes of `data` from `at` to `end`.
void MoveChunks(std::vector<char>* data, size_t at, size_t end, uint32_t delta) {
  while (at + 8 <= end) {
    const uint32_t size = Be32(data->data() + at);
    if (size < 8 || at + size > end) return;
    const std::string type(data->data() + at + 4, 4);
    if (type == "moov" || type == "trak" || type == "mdia" || type == "minf" ||
        type == "stbl") {
      MoveChunks(data, at + 8, at + size, delta);
    } else if (type == "stco") {
      const uint32_t count = Be32(data->data() + at + 12);
      for (uint32_t i = 0; i < count && at + 16 + 4 * (i + 1) <= at + size; i++) {
        char* entry = data->data() + at + 16 + 4 * i;
        PutBe32(entry, Be32(entry) + delta);
      }
    }
    at += size;
  }
}

// The clip with `pad` bytes of a free box before its pictures, its index moved to match:
// what is read with the index then ends well before the first picture. `mdat` is where
// the pictures' box starts in the padded clip.
std::vector<char> Padded(const std::vector<char>& body, uint32_t pad, size_t* mdat) {
  size_t at = 0;
  while (at + 8 <= body.size() && std::string(body.data() + at + 4, 4) != "mdat") {
    at += Be32(body.data() + at);
  }
  std::vector<char> out(body.begin(), body.begin() + static_cast<std::ptrdiff_t>(at));
  MoveChunks(&out, 0, out.size(), pad);
  std::vector<char> free_box(pad, 0);
  PutBe32(free_box.data(), pad);
  memcpy(free_box.data() + 4, "free", 4);
  out.insert(out.end(), free_box.begin(), free_box.end());
  *mdat = out.size();
  out.insert(out.end(), body.begin() + static_cast<std::ptrdiff_t>(at), body.end());
  return out;
}

// The clip as a byte stream whose reads past `limit` wait until LetReadsGo (or Close):
// Media Foundation reads the index, makes the source and the reader, and the first
// picture never comes. `alive` counts the streams not yet let go of.
std::atomic<int> alive{0};

class StallingStream : public IMFByteStream {
 public:
  StallingStream(const std::vector<char>& body, QWORD limit)
      : body_(body), limit_(limit) {
    alive++;
    released_ = CreateEventW(nullptr, TRUE, FALSE, nullptr);
  }
  ~StallingStream() {
    CloseHandle(released_);
    alive--;
  }
  void LetReadsGo() { SetEvent(released_); }
  bool waited() const { return waited_; }

  STDMETHODIMP QueryInterface(REFIID riid, void** object) override {
    if (riid == __uuidof(IUnknown) || riid == __uuidof(IMFByteStream)) {
      *object = static_cast<IMFByteStream*>(this);
      AddRef();
      return S_OK;
    }
    *object = nullptr;
    return E_NOINTERFACE;
  }
  STDMETHODIMP_(ULONG) AddRef() override { return ++refs_; }
  STDMETHODIMP_(ULONG) Release() override {
    const ULONG left = --refs_;
    if (left == 0) delete this;
    return left;
  }

  STDMETHODIMP GetCapabilities(DWORD* caps) override {
    *caps = MFBYTESTREAM_IS_READABLE | MFBYTESTREAM_IS_SEEKABLE;
    return S_OK;
  }
  STDMETHODIMP GetLength(QWORD* length) override {
    *length = body_.size();
    return S_OK;
  }
  STDMETHODIMP SetLength(QWORD) override { return E_NOTIMPL; }
  STDMETHODIMP GetCurrentPosition(QWORD* position) override {
    std::lock_guard<std::mutex> lock(mutex_);
    *position = position_;
    return S_OK;
  }
  STDMETHODIMP SetCurrentPosition(QWORD position) override {
    std::lock_guard<std::mutex> lock(mutex_);
    position_ = position;
    return S_OK;
  }
  STDMETHODIMP IsEndOfStream(BOOL* end) override {
    std::lock_guard<std::mutex> lock(mutex_);
    *end = position_ >= body_.size();
    return S_OK;
  }
  STDMETHODIMP Read(BYTE* to, ULONG size, ULONG* read) override {
    QWORD at;
    {
      std::lock_guard<std::mutex> lock(mutex_);
      at = position_;
    }
    // A read that needs a byte past the limit waits whole: a short read would mean the
    // end of the file.
    if ((std::min)(at + size, static_cast<QWORD>(body_.size())) > limit_) {
      waited_ = true;
      WaitForSingleObject(released_, INFINITE);
    }
    std::lock_guard<std::mutex> lock(mutex_);
    if (closed_) return E_FAIL;
    const QWORD end = body_.size();
    const ULONG n =
        at >= end ? 0 : static_cast<ULONG>((std::min)(static_cast<QWORD>(size), end - at));
    if (n > 0) memcpy(to, body_.data() + at, n);
    position_ = at + n;
    *read = n;
    return S_OK;
  }
  STDMETHODIMP BeginRead(BYTE* to, ULONG size, IMFAsyncCallback* callback,
                         IUnknown* state) override {
    ComPtr<IMFAsyncResult> result;
    HRESULT hr = MFCreateAsyncResult(nullptr, callback, state, &result);
    if (FAILED(hr)) return hr;
    ComPtr<StallingStream> self(this);
    std::thread([self, result, to, size] {
      ULONG read = 0;
      const HRESULT status = self->Read(to, size, &read);
      {
        std::lock_guard<std::mutex> lock(self->mutex_);
        self->reads_[result.Get()] = read;
      }
      result->SetStatus(status);
      MFInvokeCallback(result.Get());
    }).detach();
    return S_OK;
  }
  STDMETHODIMP EndRead(IMFAsyncResult* result, ULONG* read) override {
    std::lock_guard<std::mutex> lock(mutex_);
    auto found = reads_.find(result);
    *read = found == reads_.end() ? 0 : found->second;
    if (found != reads_.end()) reads_.erase(found);
    return result->GetStatus();
  }
  STDMETHODIMP Write(const BYTE*, ULONG, ULONG*) override { return E_NOTIMPL; }
  STDMETHODIMP BeginWrite(const BYTE*, ULONG, IMFAsyncCallback*, IUnknown*) override {
    return E_NOTIMPL;
  }
  STDMETHODIMP EndWrite(IMFAsyncResult*, ULONG*) override { return E_NOTIMPL; }
  STDMETHODIMP Seek(MFBYTESTREAM_SEEK_ORIGIN origin, LONGLONG offset, DWORD,
                    QWORD* now) override {
    std::lock_guard<std::mutex> lock(mutex_);
    position_ = static_cast<QWORD>(origin == msoBegin ? offset : static_cast<LONGLONG>(position_) + offset);
    if (now) *now = position_;
    return S_OK;
  }
  STDMETHODIMP Flush() override { return S_OK; }
  STDMETHODIMP Close() override {
    {
      std::lock_guard<std::mutex> lock(mutex_);
      closed_ = true;
    }
    SetEvent(released_);
    return S_OK;
  }

 private:
  std::atomic<ULONG> refs_{1};
  std::mutex mutex_;
  std::vector<char> body_;
  QWORD limit_;
  QWORD position_ = 0;
  bool closed_ = false;
  std::atomic<bool> waited_{false};
  HANDLE released_;
  std::map<IMFAsyncResult*, ULONG> reads_;
};

// Waits up to `seconds` for `done`.
template <typename F>
bool Within(double seconds, F done) {
  const auto until = Clock::now() + std::chrono::duration<double>(seconds);
  while (Clock::now() < until) {
    if (done()) return true;
    Sleep(50);
  }
  return done();
}

}  // namespace

int main(int argc, char** argv) {
  if (argc < 3) {
    std::printf("usage: probe_test <clip.mp4> <scratch folder>\n");
    return 2;
  }
  // Each line as it comes, so what ran before a hang or a crash is seen.
  setvbuf(stdout, nullptr, _IONBF, 0);
  const std::string clip = argv[1];
  const std::string scratch = argv[2];
  // A third argument runs only the part of that name: silent, stalls or six (all: all).
  const std::string only = argc > 3 ? argv[3] : "";
  auto part = [&only](const char* name) { return only.empty() || only == "all" || only == name; };
  WSADATA wsa;
  WSAStartup(MAKEWORD(2, 2), &wsa);
  const auto body = ReadAll(clip);
  Check(body.size() > 5000, "the clip is there");

  // The padded clip is still a video.
  {
    size_t mdat = 0;
    const auto padded = Padded(body, 4 << 20, &mdat);
    const std::string path = scratch + "\\padded.mp4";
    WriteAll(path, padded.data(), padded.size());
    Check(Run(Wide(path), std::chrono::milliseconds(10000)).ok,
          "the clip with 4 MiB before its pictures is read too");
  }

  // A video gives its first picture, size and length, and is let go of.
  const std::string copy = scratch + "\\clip.mp4";
  WriteAll(copy, body.data(), body.size());
  Timed normal = Run(Wide(copy), std::chrono::milliseconds(10000));
  std::printf("      clip: %.2f s, %dx%d, %lld ms, JPEG of %zu bytes\n", normal.seconds,
              normal.probe.width, normal.probe.height,
              static_cast<long long>(normal.probe.duration_ms),
              normal.probe.jpeg.size());
  Check(normal.ok && normal.probe.width == 480 && normal.probe.height == 270,
        "a video gives its first picture and size");
  Check(normal.probe.duration_ms >= 3900 && normal.probe.duration_ms <= 4100,
        "and its length");
  Check(normal.probe.jpeg.size() > 2 &&
            static_cast<unsigned char>(normal.probe.jpeg[0]) == 0xFF &&
            static_cast<unsigned char>(normal.probe.jpeg[1]) == 0xD8,
        "the picture is a JPEG");
  Check(Free(copy), "nothing holds the video afterwards");

  // Only the codecs the chat offers are decoded.
  {
    GUID theora = MFVideoFormat_Base;
    theora.Data1 = MAKEFOURCC('t', 'h', 'e', 'o');
    GUID wmv = MFVideoFormat_WMV3;
    Check(fogata_video::CodecAllowed(MFMediaType_Video, MFVideoFormat_H264) &&
              fogata_video::CodecAllowed(MFMediaType_Video, MFVideoFormat_VP90) &&
              fogata_video::CodecAllowed(MFMediaType_Audio, MFAudioFormat_AAC),
          "H.264, VP9 and AAC are decoded");
    Check(!fogata_video::CodecAllowed(MFMediaType_Video, theora) &&
              !fogata_video::CodecAllowed(MFMediaType_Video, wmv) &&
              !fogata_video::CodecAllowed(MFMediaType_Audio, MFAudioFormat_WMAudioV8),
          "Theora, WMV and WMA are not");
    fogata_video::HeldVideo clip_held;
    const std::vector<fogata_video::Head> any_mp4 = {
        {{0, 0, 0, 0, 'f', 't', 'y', 'p'}, {0, 0, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF}}};
    const std::string checked = scratch + "\\codecs.mp4";
    WriteAll(checked, body.data(), body.size());
    Check(clip_held.Hold(Wide(checked), any_mp4) && fogata_video::CodecsAllowed(clip_held),
          "the clip's codecs are");
  }

  // A held video is read from the file as it was checked, through byte streams over the
  // handle that checked it: whatever is put at its path afterwards is never read, and the
  // user's own applications may still write, rename or delete it meanwhile.
  {
    const std::vector<fogata_video::Head> mp4 = {
        {{0, 0, 0, 0, 'f', 't', 'y', 'p'}, {0, 0, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF}}};
    const std::vector<fogata_video::Head> mkv = {
        {{0x1A, 0x45, 0xDF, 0xA3}, {0xFF, 0xFF, 0xFF, 0xFF}}};
    const std::string folder = scratch + "\\held";
    CreateDirectoryA(folder.c_str(), nullptr);
    const std::string held_path = folder + "\\clip.mp4";
    WriteAll(held_path, body.data(), body.size());
    {
      fogata_video::HeldVideo wrong;
      Check(!wrong.Hold(Wide(held_path), mkv),
            "a file that does not begin as its name says is not held");
      fogata_video::HeldVideo none;
      Check(!none.Hold(Wide(held_path), {}), "nor one with no heads at all");
      fogata_video::HeldVideo dir;
      Check(!dir.Hold(Wide(folder), mp4), "nor a folder");
    }
    {
      // Another application has it open for writing: it is held all the same.
      HANDLE writer = CreateFileA(held_path.c_str(), GENERIC_WRITE,
                                  FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                                  nullptr, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, nullptr);
      fogata_video::HeldVideo held;
      Check(held.Hold(Wide(held_path), mp4), "a file open for writing elsewhere is held");
      if (writer != INVALID_HANDLE_VALUE) CloseHandle(writer);
    }
    {
      fogata_video::HeldVideo held;
      Check(held.Hold(Wide(held_path), mp4), "the clip is held");
      // Moved away and something else put in its place, while held.
      const std::string moved = folder + "\\moved.mp4";
      Check(MoveFileExA(held_path.c_str(), moved.c_str(), 0) != 0,
            "while held, the user can still move it");
      WriteAll(held_path, garbage_head, sizeof garbage_head);
      fogata_video::Probe probe;
      Check(fogata_video::ProbeHeld(held, 320, &probe) && probe.width == 480,
            "and what is read is the clip, not what is at its path now");
      ComPtr<IMFByteStream> stream;
      Check(held.NewStream(&stream), "a byte stream over it");
      QWORD length = 0;
      Check(stream && SUCCEEDED(stream->GetLength(&length)) && length == body.size(),
            "of the clip's length");
      Check(DeleteFileA(moved.c_str()) != 0, "the user can delete it while it is held");
      fogata_video::Probe after;
      Check(fogata_video::ProbeHeld(held, 320, &after) && after.width == 480,
            "and it is still read until let go of");
      DeleteFileA(held_path.c_str());
    }
  }

  // What is no video fails at once, and is let go of.
  std::vector<char> garbage(256 * 1024);
  unsigned state = 12345;
  for (auto& c : garbage) {
    state = state * 1103515245 + 12345;
    c = static_cast<char>(state >> 16);
  }
  const std::string junk = scratch + "\\garbage.mp4";
  WriteAll(junk, garbage.data(), garbage.size());
  Timed bad = Run(Wide(junk), std::chrono::milliseconds(10000));
  std::printf("      garbage: %.2f s\n", bad.seconds);
  Check(!bad.ok && bad.seconds < 2, "a file of garbage fails within 2 s");
  Check(Free(junk), "nothing holds the garbage afterwards");

  // A video cut short: its index promises pictures that are not there.
  const std::string cut = scratch + "\\cut.mp4";
  WriteAll(cut, body.data(), 4807 + 200);
  Timed shortened = Run(Wide(cut), std::chrono::milliseconds(10000));
  std::printf("      cut short: %.2f s, %s\n", shortened.seconds,
              shortened.ok ? "read" : "not read");
  Check(!shortened.ok && shortened.seconds < 2, "a video cut short fails within 2 s");
  Check(Free(cut), "nothing holds the video cut short afterwards");

  // A file that never comes: the open is given up at its time.
  std::printf("      silent server...\n");
  if (part("silent")) {
    SilentServer server;
    const std::wstring url =
        L"http://127.0.0.1:" + std::to_wstring(server.port()) + L"/silent.mp4";
    Timed silent = Run(url, std::chrono::milliseconds(3000));
    std::printf("      silent server: %.2f s, asked: %d\n", silent.seconds,
                server.asked() ? 1 : 0);
    Check(server.asked(), "the silent server was asked");
    Check(!silent.ok && silent.seconds >= 2.9 && silent.seconds < 4.5,
          "a file that never comes is given up at its time (3 s)");
    Check(Within(5, [&] { return server.closed(); }),
          "and its connection is closed within 5 s");
  }

  // A file that stops coming, while its index is read (`at_open`) or once the reader asks
  // for the first picture: the probe gives up at its time, cancels what is pending and,
  // once the stalled read ends, everything is let go of.
  {
    size_t mdat = 0;
    const auto padded = Padded(body, 4 << 20, &mdat);
    auto* stream = new StallingStream(padded, padded.size());
    fogata_video::Probe probe;
    Check(fogata_video::ProbeVideoStream(stream, L"padded.mp4", 320, &probe,
                                         std::chrono::milliseconds(10000)) &&
              probe.width == 480,
          "the padded clip is read through a stream that never waits");
    stream->Release();
    Check(Within(5, [] { return alive == 0; }), "and the stream is let go of");
  }
  for (const bool at_open : {true, false}) {
    const char* what = at_open ? "stalling open" : "stalling first picture";
    std::printf("      %s...\n", what);
    if (!part("stalls")) continue;
    size_t mdat = 0;
    const auto padded = Padded(body, 4 << 20, &mdat);
    // The index ends at 4807; with 4 MiB before the pictures, the index is read whole
    // and the reads of the first picture wait.
    auto* stream = at_open ? new StallingStream(body, 4807 + 64)
                           : new StallingStream(padded, mdat + 8);
    const auto start = Clock::now();
    fogata_video::Probe probe;
    const bool ok =
        fogata_video::ProbeVideoStream(stream, L"stalls.mp4", 320, &probe,
                                       std::chrono::milliseconds(3000));
    const double seconds = std::chrono::duration<double>(Clock::now() - start).count();
    std::printf("      %s: %.2f s, a read waited: %d\n", what, seconds,
                stream->waited() ? 1 : 0);
    Check(stream->waited(), "a read waited");
    Check(!ok && seconds >= 2.9 && seconds < 5.5,
          "the probe gives up at its time (3 s, and at most 2 s to cancel)");
    stream->LetReadsGo();
    stream->Release();
    Check(Within(5, [] { return alive == 0; }),
          "everything is let go of within 5 s once the read ends");
  }

  // Several at once, each on its own thread as the plugin runs them, and the same file
  // still read afterwards.
  std::printf("      six at once...\n");
  if (part("six")) {
    std::vector<std::thread> threads;
    std::atomic<int> read{0};
    for (int i = 0; i < 6; i++) {
      threads.emplace_back([&] {
        if (Run(Wide(copy), std::chrono::milliseconds(10000)).ok) read++;
      });
    }
    for (auto& t : threads) t.join();
    Check(read == 6, "six probes on threads of their own all read the video");
    Check(Free(copy), "nothing holds the video after them");
  }

  WSACleanup();
  std::printf("%s (%d failed)\n", failures == 0 ? "all passed" : "FAILED", failures);
  return failures == 0 ? 0 : 1;
}
