#include "fogata_video_plugin.h"
#include "video_probe.h"

// This must be included before many other Windows headers.
#include <windows.h>

#include <dxgiformat.h>
#include <mfapi.h>
#include <mfidl.h>
#include <mfmediaengine.h>
#include <mfreadwrite.h>
#include <mmdeviceapi.h>
#include <propidl.h>
#include <shlwapi.h>
#include <wincodec.h>
#include <wrl/client.h>

#include <flutter/method_channel.h>
#include <flutter/plugin_registrar_windows.h>
#include <flutter/standard_method_codec.h>
#include <flutter/texture_registrar.h>

#include <algorithm>
#include <atomic>
#include <cmath>
#include <cstdint>
#include <cstdlib>
#include <optional>
#include <condition_variable>
#include <deque>
#include <functional>
#include <map>
#include <memory>
#include <mutex>
#include <string>
#include <thread>
#include <vector>

using Microsoft::WRL::ComPtr;

namespace fogata_video {

namespace {

// Asks the platform thread to answer a finished probe or opening (its job in wparam).
constexpr UINT kProbeDone = WM_APP + 0x4f56;

std::wstring Wide(const std::string& text) {
  if (text.empty()) return std::wstring();
  const int n = MultiByteToWideChar(CP_UTF8, 0, text.data(),
                                    static_cast<int>(text.size()), nullptr, 0);
  std::wstring out(n, L'\0');
  MultiByteToWideChar(CP_UTF8, 0, text.data(), static_cast<int>(text.size()),
                      out.data(), n);
  return out;
}

// The heads the application gives for a video (see video_head.h): a list of maps of
// "bytes" and "mask". None when they are missing or not of that shape.
std::vector<Head> HeadsOf(const flutter::EncodableValue* value) {
  std::vector<Head> heads;
  const auto* list =
      value == nullptr ? nullptr : std::get_if<flutter::EncodableList>(value);
  if (list == nullptr) return heads;
  for (const auto& item : *list) {
    const auto* map = std::get_if<flutter::EncodableMap>(&item);
    if (map == nullptr) continue;
    auto bytes = map->find(flutter::EncodableValue("bytes"));
    auto mask = map->find(flutter::EncodableValue("mask"));
    if (bytes == map->end() || mask == map->end()) continue;
    const auto* b = std::get_if<std::vector<uint8_t>>(&bytes->second);
    const auto* m = std::get_if<std::vector<uint8_t>>(&mask->second);
    if (b == nullptr || m == nullptr) continue;
    heads.push_back({*b, *m});
  }
  return heads;
}

// IMFMediaEngineAudioEndpointId (mfmediaengine.h, Windows 10), declared here under a name of
// its own so the build does not depend on which SDK's headers declare it.
MIDL_INTERFACE("7a3bac98-0e76-49fb-8c20-8a86fd98eaf2")
EngineAudioEndpoint : public IUnknown {
 public:
  virtual HRESULT STDMETHODCALLTYPE SetAudioEndpointId(LPCWSTR endpoint_id) = 0;
  virtual HRESULT STDMETHODCALLTYPE GetAudioEndpointId(LPWSTR* endpoint_id) = 0;
};

// The connected output `chosen` names, as the voice engine lists outputs (by endpoint id,
// or by name when it has no id): its endpoint id, or empty (the system's default) when no
// such output is connected now.
std::wstring OutputEndpoint(const std::wstring& chosen) {
  if (chosen.empty()) return std::wstring();
  ComPtr<IMMDeviceEnumerator> enumerator;
  ComPtr<IMMDeviceCollection> outputs;
  UINT count = 0;
  if (FAILED(CoCreateInstance(__uuidof(MMDeviceEnumerator), nullptr,
                              CLSCTX_INPROC_SERVER, IID_PPV_ARGS(&enumerator))) ||
      FAILED(enumerator->EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE,
                                            &outputs)) ||
      FAILED(outputs->GetCount(&count))) {
    return std::wstring();
  }
  // PKEY_Device_FriendlyName, spelled out: the SDK's header only declares it.
  const PROPERTYKEY friendly_name = {
      {0xa45c254e, 0xdf1c, 0x4efd,
       {0x80, 0x20, 0x67, 0xd1, 0x46, 0xa8, 0x50, 0xe0}},
      14};
  std::wstring by_name;
  for (UINT i = 0; i < count; i++) {
    ComPtr<IMMDevice> device;
    LPWSTR id = nullptr;
    if (FAILED(outputs->Item(i, &device)) || FAILED(device->GetId(&id))) {
      continue;
    }
    const std::wstring endpoint(id);
    CoTaskMemFree(id);
    if (endpoint == chosen) return endpoint;
    ComPtr<IPropertyStore> properties;
    PROPVARIANT name;
    PropVariantInit(&name);
    if (by_name.empty() &&
        SUCCEEDED(device->OpenPropertyStore(STGM_READ, &properties)) &&
        SUCCEEDED(properties->GetValue(friendly_name, &name)) &&
        name.vt == VT_LPWSTR && name.pwszVal != nullptr &&
        chosen == name.pwszVal) {
      by_name = endpoint;
    }
    PropVariantClear(&name);
  }
  return by_name;
}

// What the media engine's events told.
struct Heard {
  std::mutex mutex;
  std::string error;
};

// Hears the media engine's events: the end and errors.
class EngineEvents : public IMFMediaEngineNotify {
 public:
  explicit EngineEvents(std::function<void(DWORD, DWORD_PTR)> on_event)
      : on_event_(std::move(on_event)) {}

  STDMETHODIMP QueryInterface(REFIID riid, void** object) override {
    if (riid == __uuidof(IUnknown) || riid == __uuidof(IMFMediaEngineNotify)) {
      *object = static_cast<IMFMediaEngineNotify*>(this);
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
  STDMETHODIMP EventNotify(DWORD event, DWORD_PTR param1, DWORD) override {
    on_event_(event, param1);
    return S_OK;
  }

 private:
  std::atomic<ULONG> refs_{1};
  std::function<void(DWORD, DWORD_PTR)> on_event_;
};

}  // namespace

// Probes running, by job: their result channel, and their answer once done.
struct Probes {
  std::mutex mutex;
  int next_job = 0;
  // Running now, and the most at once: one more is answered at once as unread. A probe
  // counts until its thread is done, even once nobody waits for it.
  int running = 0;
  static constexpr int kAtOnce = 2;
  std::map<int, std::unique_ptr<flutter::MethodResult<flutter::EncodableValue>>>
      waiting;
  std::map<int, flutter::EncodableValue> done;
  // Openings for a player: the file opened and checked, or none, and where its sound goes.
  std::map<int, std::pair<std::unique_ptr<HeldVideo>, std::wstring>> opened;
  // Openings whose thread has not come back yet, and the most at once: a file that never
  // opens (on a network share that hangs) holds its thread.
  int opening = 0;
  static constexpr int kOpeningMost = 8;
};

// A video playing: Media Foundation's media engine, on a thread of its own, hands each
// picture over through a WIC bitmap.
class Player {
 public:
  // `output` is the output device chosen in the application, as the voice engine names
  // it; empty for the system's default. `video` was opened and checked (HeldVideo::Hold).
  Player(std::unique_ptr<HeldVideo> video, std::wstring output,
         flutter::TextureRegistrar* textures)
      : textures_(textures), held_(std::move(video)) {
    texture_ = std::make_unique<flutter::TextureVariant>(
        flutter::PixelBufferTexture([this](size_t, size_t) {
          return CopyPixels();
        }));
    id_ = textures_->RegisterTexture(texture_.get());
    worker_ = std::thread([this, output]() { Run(output); });
  }

  int64_t id() const { return id_; }

  // Runs `command` on the engine's thread.
  void Do(std::function<void(IMFMediaEngine*)> command) {
    {
      std::lock_guard<std::mutex> lock(mutex_);
      commands_.push_back(std::move(command));
    }
    wake_.notify_one();
  }

  flutter::EncodableMap State() {
    std::lock_guard<std::mutex> lock(mutex_);
    flutter::EncodableMap state = {
        {flutter::EncodableValue("positionMs"),
         flutter::EncodableValue(position_ms_)},
        {flutter::EncodableValue("durationMs"),
         flutter::EncodableValue(duration_ms_)},
        {flutter::EncodableValue("playing"), flutter::EncodableValue(playing_)},
        {flutter::EncodableValue("ended"), flutter::EncodableValue(ended_)},
        {flutter::EncodableValue("width"),
         flutter::EncodableValue(static_cast<int>(width_))},
        {flutter::EncodableValue("height"),
         flutter::EncodableValue(static_cast<int>(height_))},
    };
    if (!error_.empty()) {
      state[flutter::EncodableValue("error")] = flutter::EncodableValue(error_);
    }
    return state;
  }

  // Stops the engine, then lets the texture and this player go.
  void Close() {
    stop_ = true;
    wake_.notify_one();
    if (worker_.joinable()) worker_.join();
    textures_->UnregisterTexture(id_, [this]() { delete this; });
  }

 private:
  const FlutterDesktopPixelBuffer* CopyPixels() {
    std::lock_guard<std::mutex> lock(mutex_);
    if (!latest_.empty()) {
      shown_.swap(latest_);
      latest_.clear();
    }
    if (shown_.empty()) return nullptr;
    pixels_.buffer = shown_.data();
    pixels_.width = shown_width_;
    pixels_.height = shown_height_;
    return &pixels_;
  }

  void Run(const std::wstring& output) {
    MediaThread media;
    // Media Foundation reads the file through byte streams over the handle that checked it.
    const HeldVideo& held = *held_;
    const std::wstring& asked = held.name();
    ComPtr<IMFByteStream> stream;
    if (!held.NewStream(&stream)) {
      std::lock_guard<std::mutex> lock(mutex_);
      error_ = "it cannot be read";
      return;
    }
    ComPtr<IMFMediaEngineClassFactory> factory;
    ComPtr<IMFAttributes> attrs;
    ComPtr<IMFMediaEngine> engine;
    ComPtr<IWICImagingFactory> wic;
    // The engine may tell of an event after Shutdown, when this player may be gone: what
    // it tells goes to a place of its own, read here.
    auto heard = std::make_shared<Heard>();
    auto* events = new EngineEvents([heard](DWORD event, DWORD_PTR param) {
      std::lock_guard<std::mutex> lock(heard->mutex);
      if (event == MF_MEDIA_ENGINE_EVENT_ERROR) {
        heard->error = "cannot play it (" + std::to_string(param) + ")";
      }
    });
    // Only a name for the stream, whose extension tells the engine the kind: nothing is
    // opened by it.
    const size_t dot = asked.find_last_of(L'.');
    const std::wstring ext =
        dot == std::wstring::npos || asked.find_first_of(L"\\/", dot) != std::wstring::npos
            ? L".mp4"
            : asked.substr(dot);
    BSTR source = SysAllocString((L"video" + ext).c_str());
    ComPtr<IMFMediaEngineEx> engine_ex;
    bool ok =
        source != nullptr &&
        SUCCEEDED(CoCreateInstance(CLSID_MFMediaEngineClassFactory, nullptr,
                                   CLSCTX_INPROC_SERVER,
                                   IID_PPV_ARGS(&factory))) &&
        SUCCEEDED(CoCreateInstance(CLSID_WICImagingFactory, nullptr,
                                   CLSCTX_INPROC_SERVER, IID_PPV_ARGS(&wic))) &&
        SUCCEEDED(MFCreateAttributes(&attrs, 2)) &&
        SUCCEEDED(attrs->SetUnknown(MF_MEDIA_ENGINE_CALLBACK, events)) &&
        SUCCEEDED(attrs->SetUINT32(MF_MEDIA_ENGINE_VIDEO_OUTPUT_FORMAT,
                                   DXGI_FORMAT_B8G8R8A8_UNORM)) &&
        SUCCEEDED(factory->CreateInstance(0, attrs.Get(), &engine));
    if (ok) {
      // The sound goes to the chosen output while it is connected, else to the system's
      // default. The engine takes it for the source set next.
      const std::wstring endpoint = OutputEndpoint(output);
      ComPtr<EngineAudioEndpoint> choice;
      if (!endpoint.empty() && SUCCEEDED(engine.As(&choice))) {
        choice->SetAudioEndpointId(endpoint.c_str());
      }
      ok = SUCCEEDED(engine.As(&engine_ex)) &&
           SUCCEEDED(engine_ex->SetSourceFromByteStream(stream.Get(), source));
    }
    if (source != nullptr) SysFreeString(source);
    events->Release();
    if (!ok) {
      std::lock_guard<std::mutex> lock(mutex_);
      if (error_.empty()) error_ = "cannot play it";
      return;
    }
    ComPtr<IMFByteStream> for_turns;
    const Turns turns =
        held.NewStream(&for_turns) ? TurnsOf(for_turns.Get()) : Turns();
    ComPtr<IWICBitmap> bitmap;
    while (!stop_) {
      std::deque<std::function<void(IMFMediaEngine*)>> commands;
      {
        std::unique_lock<std::mutex> lock(mutex_);
        wake_.wait_for(lock, std::chrono::milliseconds(8),
                       [this]() { return stop_ || !commands_.empty(); });
        commands.swap(commands_);
      }
      for (auto& command : commands) command(engine.Get());
      LONGLONG pts = 0;
      DWORD w = 0;
      DWORD h = 0;
      if (engine->OnVideoStreamTick(&pts) == S_OK &&
          SUCCEEDED(engine->GetNativeVideoSize(&w, &h)) && w > 0 && h > 0) {
        if (!SizeAllowed(w, h)) {
          // Too large to play: it stops here, and says why.
          std::lock_guard<std::mutex> lock(mutex_);
          error_ = "its pictures are too large to play";
          playing_ = false;
          ended_ = true;
          break;
        }
        UINT bw = 0;
        UINT bh = 0;
        if (bitmap) bitmap->GetSize(&bw, &bh);
        if (!bitmap || bw != w || bh != h) {
          bitmap.Reset();
          wic->CreateBitmap(w, h, GUID_WICPixelFormat32bppBGRA,
                            WICBitmapCacheOnLoad, &bitmap);
        }
        MFVideoNormalizedRect from = {0.0f, 0.0f, 1.0f, 1.0f};
        RECT to = {0, 0, static_cast<LONG>(w), static_cast<LONG>(h)};
        MFARGB black = {0, 0, 0, 255};
        // Turned here, unless the engine hands them over turned already.
        const bool stored = w == turns.width && h == turns.height;
        if (bitmap &&
            SUCCEEDED(engine->TransferVideoFrame(bitmap.Get(), &from, &to,
                                                 &black))) {
          TakePicture(bitmap.Get(), w, h, stored ? turns.turns : 0);
        }
        if (stored && turns.turns % 2 == 1) std::swap(w, h);
      }
      const double position = engine->GetCurrentTime();
      const double duration = engine->GetDuration();
      std::string error;
      {
        std::lock_guard<std::mutex> lock(heard->mutex);
        error = heard->error;
      }
      std::lock_guard<std::mutex> lock(mutex_);
      if (!error.empty() && error_.empty()) error_ = error;
      position_ms_ = static_cast<int64_t>(position * 1000);
      duration_ms_ =
          std::isfinite(duration) ? static_cast<int64_t>(duration * 1000) : 0;
      playing_ = !engine->IsPaused() && !engine->IsEnded();
      ended_ = engine->IsEnded() == TRUE;
      width_ = w > 0 ? w : width_;
      height_ = h > 0 ? h : height_;
    }
    engine->Shutdown();
  }

  // Copies the bitmap's BGRA picture as RGBA for Flutter, turned `turns` quarter turns
  // clockwise.
  void TakePicture(IWICBitmap* bitmap, DWORD w, DWORD h, int turns) {
    WICRect all = {0, 0, static_cast<INT>(w), static_cast<INT>(h)};
    ComPtr<IWICBitmapLock> lock;
    if (FAILED(bitmap->Lock(&all, WICBitmapLockRead, &lock))) return;
    UINT stride = 0;
    UINT size = 0;
    BYTE* data = nullptr;
    if (FAILED(lock->GetStride(&stride)) ||
        FAILED(lock->GetDataPointer(&size, &data)) || data == nullptr) {
      return;
    }
    // In 64 bits: every row must hold the picture's width and lie inside the bitmap.
    const uint64_t row = uint64_t{w} * 4;
    if (h == 0 || stride < row ||
        uint64_t{stride} * (h - 1) + row > uint64_t{size}) {
      return;
    }
    std::vector<uint8_t> rgba(static_cast<size_t>(w) * h * 4);
    for (DWORD y = 0; y < h; y++) {
      const BYTE* from = data + static_cast<size_t>(y) * stride;
      uint8_t* to = rgba.data() + static_cast<size_t>(y) * w * 4;
      for (DWORD x = 0; x < w; x++) {
        to[x * 4 + 0] = from[x * 4 + 2];
        to[x * 4 + 1] = from[x * 4 + 1];
        to[x * 4 + 2] = from[x * 4 + 0];
        to[x * 4 + 3] = 255;
      }
    }
    lock.Reset();
    UINT tw = w;
    UINT th = h;
    Turn(&rgba, &tw, &th, turns);
    {
      std::lock_guard<std::mutex> guard(mutex_);
      latest_.swap(rgba);
      shown_width_ = tw;
      shown_height_ = th;
    }
    textures_->MarkTextureFrameAvailable(id_);
  }

  flutter::TextureRegistrar* textures_;
  const std::unique_ptr<HeldVideo> held_;
  std::unique_ptr<flutter::TextureVariant> texture_;
  int64_t id_ = 0;
  std::thread worker_;
  std::atomic<bool> stop_{false};
  std::mutex mutex_;
  std::condition_variable wake_;
  std::deque<std::function<void(IMFMediaEngine*)>> commands_;
  std::vector<uint8_t> latest_;
  std::vector<uint8_t> shown_;
  size_t shown_width_ = 0;
  size_t shown_height_ = 0;
  FlutterDesktopPixelBuffer pixels_ = {};
  int64_t position_ms_ = 0;
  int64_t duration_ms_ = 0;
  bool playing_ = false;
  bool ended_ = false;
  DWORD width_ = 0;
  DWORD height_ = 0;
  std::string error_;
};

// static
void FogataVideoPlugin::RegisterWithRegistrar(
    flutter::PluginRegistrarWindows* registrar) {
  auto channel =
      std::make_unique<flutter::MethodChannel<flutter::EncodableValue>>(
          registrar->messenger(), "fogata/video",
          &flutter::StandardMethodCodec::GetInstance());
  auto plugin = std::make_unique<FogataVideoPlugin>(registrar);
  channel->SetMethodCallHandler(
      [plugin_pointer = plugin.get()](const auto& call, auto result) {
        plugin_pointer->HandleMethodCall(call, std::move(result));
      });
  registrar->AddPlugin(std::move(plugin));
}

FogataVideoPlugin::FogataVideoPlugin(flutter::PluginRegistrarWindows* registrar)
    : registrar_(registrar),
      textures_(registrar->texture_registrar()),
      probes_(std::make_shared<Probes>()) {
  window_proc_ = registrar->RegisterTopLevelWindowProcDelegate(
      [this](HWND, UINT message, WPARAM wparam,
             LPARAM) -> std::optional<LRESULT> {
        if (message != kProbeDone) return std::nullopt;
        AnswerProbe(static_cast<int>(wparam));
        return 0;
      });
}

FogataVideoPlugin::~FogataVideoPlugin() {
  registrar_->UnregisterTopLevelWindowProcDelegate(window_proc_);
  for (auto& [id, player] : players_) player->Close();
  // A probe still running finds nobody waiting for it.
  std::lock_guard<std::mutex> lock(probes_->mutex);
  probes_->waiting.clear();
}

void FogataVideoPlugin::AnswerProbe(int job) {
  std::unique_ptr<flutter::MethodResult<flutter::EncodableValue>> result;
  flutter::EncodableValue answer;
  {
    std::lock_guard<std::mutex> lock(probes_->mutex);
    auto waiting = probes_->waiting.find(job);
    auto opened = probes_->opened.find(job);
    if (waiting != probes_->waiting.end() && opened != probes_->opened.end()) {
      // An opening: the player is made here, on the platform thread, or there is none.
      result = std::move(waiting->second);
      auto [video, output] = std::move(opened->second);
      probes_->waiting.erase(waiting);
      probes_->opened.erase(opened);
      if (video == nullptr) {
        result->Success(flutter::EncodableValue());
        return;
      }
      auto* player = new Player(std::move(video), std::move(output), textures_);
      players_[player->id()] = player;
      result->Success(flutter::EncodableValue(player->id()));
      return;
    }
    auto done = probes_->done.find(job);
    if (waiting == probes_->waiting.end() || done == probes_->done.end()) return;
    result = std::move(waiting->second);
    answer = std::move(done->second);
    probes_->waiting.erase(waiting);
    probes_->done.erase(done);
  }
  result->Success(answer);
}

void FogataVideoPlugin::HandleMethodCall(
    const flutter::MethodCall<flutter::EncodableValue>& call,
    std::unique_ptr<flutter::MethodResult<flutter::EncodableValue>> result) {
  const auto* args = std::get_if<flutter::EncodableMap>(call.arguments());
  auto arg = [args](const char* key) -> const flutter::EncodableValue* {
    if (args == nullptr) return nullptr;
    auto found = args->find(flutter::EncodableValue(key));
    return found == args->end() ? nullptr : &found->second;
  };
  auto number = [&arg](const char* key, int64_t fallback) -> int64_t {
    const auto* v = arg(key);
    if (v == nullptr) return fallback;
    if (const auto* i = std::get_if<int32_t>(v)) return *i;
    if (const auto* l = std::get_if<int64_t>(v)) return *l;
    return fallback;
  };
  const std::string& method = call.method_name();
  if (method == "probe" || method == "open") {
    const auto* path_value = arg("path");
    const auto* path = path_value == nullptr
                           ? nullptr
                           : std::get_if<std::string>(path_value);
    if (path == nullptr) {
      result->Error("args", "no path");
      return;
    }
    if (method == "open") {
      const auto* output_value = arg("output");
      const auto* output = output_value == nullptr
                               ? nullptr
                               : std::get_if<std::string>(output_value);
      std::wstring sound = output == nullptr ? std::wstring() : Wide(*output);
      // The file is opened and checked on a thread of its own, which a file slow to open
      // (on a network share) would otherwise hold up, and the player made once it answers:
      // none for a file that is not the kind of video its name says.
      flutter::FlutterView* view = registrar_->GetView();
      const HWND window =
          view == nullptr ? nullptr : GetAncestor(view->GetNativeWindow(), GA_ROOT);
      int job;
      {
        std::lock_guard<std::mutex> lock(probes_->mutex);
        if (window == nullptr || probes_->opening >= Probes::kOpeningMost) {
          result->Success(flutter::EncodableValue());
          return;
        }
        probes_->opening++;
        job = probes_->next_job++;
        probes_->waiting[job] = std::move(result);
      }
      std::thread([probes = probes_, job, wide = Wide(*path), heads = HeadsOf(arg("heads")),
                   sound = std::move(sound), window]() {
        auto video = std::make_unique<HeldVideo>();
        // Not the kind of video its name says, or a codec the chat does not offer: no
        // player.
        if (!video->Hold(wide, heads) || !CodecsAllowed(*video)) video.reset();
        {
          std::lock_guard<std::mutex> lock(probes->mutex);
          probes->opening--;
          if (probes->waiting.count(job) == 0) return;
          probes->opened[job] = {std::move(video), sound};
        }
        PostMessage(window, kProbeDone, static_cast<WPARAM>(job), 0);
      }).detach();
      return;
    }
    int job;
    {
      std::lock_guard<std::mutex> lock(probes_->mutex);
      if (probes_->running >= Probes::kAtOnce) {
        result->Success(flutter::EncodableValue());
        return;
      }
      probes_->running++;
      job = probes_->next_job++;
      probes_->waiting[job] = std::move(result);
    }
    const int max_width = static_cast<int>(number("maxWidth", 480));
    // The view sits in its top-level window only after plugins are registered, so the
    // window is looked up here. Without a window there is nobody to answer through.
    flutter::FlutterView* view = registrar_->GetView();
    const HWND window =
        view == nullptr ? nullptr : GetAncestor(view->GetNativeWindow(), GA_ROOT);
    if (window == nullptr) {
      std::unique_ptr<flutter::MethodResult<flutter::EncodableValue>> waiting;
      {
        std::lock_guard<std::mutex> lock(probes_->mutex);
        waiting = std::move(probes_->waiting[job]);
        probes_->waiting.erase(job);
        probes_->running--;
      }
      waiting->Success(flutter::EncodableValue());
      return;
    }
    std::thread([probes = probes_, job, wide = Wide(*path),
                 heads = HeadsOf(arg("heads")), max_width, window]() {
      Probe probe;
      flutter::EncodableValue answer;
      // Read from the file opened and checked, as a player reads it.
      HeldVideo held;
      if (held.Hold(wide, heads) && ProbeHeld(held, max_width, &probe)) {
        answer = flutter::EncodableValue(flutter::EncodableMap{
            {flutter::EncodableValue("thumb"),
             flutter::EncodableValue(probe.jpeg)},
            {flutter::EncodableValue("width"),
             flutter::EncodableValue(probe.width)},
            {flutter::EncodableValue("height"),
             flutter::EncodableValue(probe.height)},
            {flutter::EncodableValue("durationMs"),
             flutter::EncodableValue(probe.duration_ms)},
        });
      }
      {
        std::lock_guard<std::mutex> lock(probes->mutex);
        probes->running--;
        if (probes->waiting.count(job) == 0) return;
        probes->done[job] = std::move(answer);
      }
      PostMessage(window, kProbeDone, static_cast<WPARAM>(job), 0);
    }).detach();
    return;
  }
  const int64_t id = number("id", -1);
  auto found = players_.find(id);
  if (found == players_.end()) {
    result->Error("player", "no such player");
    return;
  }
  Player* player = found->second;
  if (method == "play") {
    player->Do([](IMFMediaEngine* engine) {
      if (engine->IsEnded()) engine->SetCurrentTime(0);
      engine->Play();
    });
  } else if (method == "pause") {
    player->Do([](IMFMediaEngine* engine) { engine->Pause(); });
  } else if (method == "seek") {
    const double seconds = static_cast<double>(number("ms", 0)) / 1000.0;
    player->Do(
        [seconds](IMFMediaEngine* engine) { engine->SetCurrentTime(seconds); });
  } else if (method == "volume") {
    const auto* v = arg("volume");
    const double* volume = v == nullptr ? nullptr : std::get_if<double>(v);
    const double level = volume == nullptr ? 1.0 : *volume;
    player->Do([level](IMFMediaEngine* engine) { engine->SetVolume(level); });
  } else if (method == "state") {
    result->Success(flutter::EncodableValue(player->State()));
    return;
  } else if (method == "close") {
    players_.erase(found);
    player->Close();
  } else {
    result->NotImplemented();
    return;
  }
  result->Success();
}

}  // namespace fogata_video
