// The sound a shared window brings (Windows): finds the window titled argv[1], the id the
// plugin lists it by, and takes the sound loopback capture gives for that id while
// another program plays (nothing of it may come), then once the window's own program
// plays too (after this creates the file argv[2], which that program waits for). A
// whole screen's capture ("0") first shows the other program can be heard at all, and
// last, with the other program stopped (argv[3], its process id), the window's own.
// Built and run by run_tests.ps1; prints the loudest sample of each and exits 1 when the
// shared window brought another program's sound or not its own.

#include <windows.h>

#include <atomic>
#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <string>

#include "application_loopback_capturer.h"
#include "libwebrtc.h"
#include "rtc_desktop_device.h"
#include "rtc_desktop_media_list.h"
#include "rtc_peerconnection_factory.h"

using namespace flutter_webrtc_plugin;
using namespace libwebrtc;

namespace {

// Keeps the loudest 16-bit sample it is given.
class Loudest : public RTCAudioSource {
 public:
  void CaptureFrame(const void* data, int bits, int, size_t channels,
                    size_t frames) override {
    if (bits != 16) return;
    const int16_t* samples = static_cast<const int16_t*>(data);
    int most = 0;
    for (size_t i = 0; i < frames * channels; i++) {
      most = max(most, std::abs(static_cast<int>(samples[i])));
    }
    int seen = peak.load();
    while (most > seen && !peak.compare_exchange_weak(seen, most)) {
    }
    taken += static_cast<long>(frames);
  }
  SourceType GetSourceType() const override { return kCustom; }

  std::atomic<int> peak{0};
  std::atomic<long> taken{0};
};

int Measure(const std::string& id, int ms, const char* what) {
  std::printf("%s: starting\n", what);
  auto capturer = CreateLoopbackCapturer(id, id != "0");
  scoped_refptr<Loudest> source = new RefCountedObject<Loudest>();
  if (!capturer || !capturer->Start(source)) {
    std::printf("%s: capture did not start\n", what);
    return -1;
  }
  std::printf("%s: started\n", what);
  Sleep(ms);
  capturer->Stop();
  std::printf("%s (id %s): %ld frames, loudest %d\n", what, id.c_str(),
              source->taken.load(), source->peak.load());
  return source->peak.load();
}

// Says where a crash happened, should one.
LONG WINAPI Crashed(EXCEPTION_POINTERS* e) {
  HMODULE module = nullptr;
  char name[MAX_PATH] = "?";
  void* at = e->ExceptionRecord->ExceptionAddress;
  if (GetModuleHandleExA(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS |
                             GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                         static_cast<LPCSTR>(at), &module)) {
    GetModuleFileNameA(module, name, MAX_PATH);
  }
  std::printf("crash 0x%lx at %s+0x%llx\n", e->ExceptionRecord->ExceptionCode, name,
              static_cast<unsigned long long>(static_cast<char*>(at) -
                                              reinterpret_cast<char*>(module)));
  return EXCEPTION_EXECUTE_HANDLER;
}

}  // namespace

int main(int argc, char** argv) {
  if (argc < 3) return 2;
  std::setvbuf(stdout, nullptr, _IONBF, 0);
  SetUnhandledExceptionFilter(Crashed);
  // As the application's own main thread.
  CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED);
  const std::string title = argv[1];
  HWND window = FindWindowA(nullptr, title.c_str());
  DWORD pid = 0;
  GetWindowThreadProcessId(window, &pid);
  std::printf("window %s: HWND %llu, process %lu\n", title.c_str(),
              static_cast<unsigned long long>(reinterpret_cast<uintptr_t>(window)), pid);
  const std::string id = std::to_string(reinterpret_cast<uintptr_t>(window));
  const int all = Measure("0", 3000, "a whole screen, another program playing");
  const int others = Measure(id, 3000, "the window, another program playing");
  std::ofstream(argv[2]) << "play";
  Sleep(1000);
  const int own = Measure(id, 3000, "the window, its own program playing too");
  // With the other program stopped (argv[3], its process id), a whole screen's capture
  // hears the window's own program alone: it does play.
  int alone = -1;
  if (argc > 3) {
    HANDLE other = OpenProcess(PROCESS_TERMINATE | SYNCHRONIZE, FALSE,
                               static_cast<DWORD>(std::strtoul(argv[3], nullptr, 10)));
    if (other != nullptr) {
      TerminateProcess(other, 0);
      WaitForSingleObject(other, 5000);
      CloseHandle(other);
    }
    Sleep(500);
    alone = Measure("0", 3000, "a whole screen, only the window's program playing");
  }
  // The id the plugin lists the window by, as the application shares it.
  LibWebRTC::Initialize();
  std::string listed;
  {
    scoped_refptr<RTCPeerConnectionFactory> factory =
        LibWebRTC::CreateRTCPeerConnectionFactory();
    factory->Initialize();
    std::printf("factory ready\n");
    scoped_refptr<RTCDesktopMediaList> list =
        factory->GetDesktopDevice()->GetDesktopMediaList(kWindow);
    list->UpdateSourceList(true, false);
    std::printf("%d windows listed\n", list->GetSourceCount());
    for (int i = 0; i < list->GetSourceCount(); i++) {
      scoped_refptr<MediaSource> source = list->GetSource(i);
      if (source->name().std_string() == title) listed = source->id().std_string();
    }
    factory->Terminate();
  }
  // The plugin shares the window by the id it lists it by: the capture above used the
  // window's handle, which must be that id.
  std::printf("listed as %s\n", listed.empty() ? "(not listed)" : listed.c_str());
  const bool ok = all > 1000 && others < 30 && own > 1000 && listed == id &&
                  (alone < 0 || alone > 1000);
  std::printf(ok ? "passed\n" : "failed\n");
  return ok ? 0 : 1;
}
