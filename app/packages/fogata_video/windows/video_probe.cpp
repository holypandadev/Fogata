#include "video_probe.h"

#include <mferror.h>
#include <propidl.h>
#include <wincodec.h>
#include <wrl/client.h>

#include <algorithm>
#include <atomic>
#include <condition_variable>
#include <cstring>
#include <mutex>
#include <system_error>
#include <thread>

using Microsoft::WRL::ComPtr;

namespace fogata_video {

MediaThread::MediaThread() {
  CoInitializeEx(nullptr, COINIT_MULTITHREADED);
  // Started once for the process and never shut down: work a probe gave up on may still
  // be ending in Media Foundation's own threads, which a shutdown would pull the
  // platform from under (the next probe then crashed).
  static std::once_flag started;
  std::call_once(started, [] { MFStartup(MF_VERSION, MFSTARTUP_LITE); });
}

MediaThread::~MediaThread() { CoUninitialize(); }

namespace {

// A JPEG of `width` x `height` BGRX pixels, made at most `max_width` wide.
bool EncodeJpeg(const std::vector<uint8_t>& bgrx, UINT width, UINT height,
                int max_width, std::vector<uint8_t>* out) {
  ComPtr<IWICImagingFactory> wic;
  if (FAILED(CoCreateInstance(CLSID_WICImagingFactory, nullptr,
                              CLSCTX_INPROC_SERVER, IID_PPV_ARGS(&wic)))) {
    return false;
  }
  ComPtr<IWICBitmap> bitmap;
  if (FAILED(wic->CreateBitmapFromMemory(
          width, height, GUID_WICPixelFormat32bppBGR, width * 4,
          static_cast<UINT>(bgrx.size()), const_cast<BYTE*>(bgrx.data()),
          &bitmap))) {
    return false;
  }
  const UINT w = width > static_cast<UINT>(max_width) ? max_width : width;
  const UINT h = (std::max)(1u, height * w / width);
  ComPtr<IWICBitmapScaler> scaler;
  ComPtr<IWICFormatConverter> converter;
  if (FAILED(wic->CreateBitmapScaler(&scaler)) ||
      FAILED(scaler->Initialize(bitmap.Get(), w, h,
                                WICBitmapInterpolationModeFant)) ||
      FAILED(wic->CreateFormatConverter(&converter)) ||
      FAILED(converter->Initialize(scaler.Get(), GUID_WICPixelFormat24bppBGR,
                                   WICBitmapDitherTypeNone, nullptr, 0,
                                   WICBitmapPaletteTypeCustom))) {
    return false;
  }
  ComPtr<IStream> stream;
  ComPtr<IWICBitmapEncoder> encoder;
  ComPtr<IWICBitmapFrameEncode> frame;
  ComPtr<IPropertyBag2> props;
  if (FAILED(CreateStreamOnHGlobal(nullptr, TRUE, &stream)) ||
      FAILED(wic->CreateEncoder(GUID_ContainerFormatJpeg, nullptr, &encoder)) ||
      FAILED(encoder->Initialize(stream.Get(), WICBitmapEncoderNoCache)) ||
      FAILED(encoder->CreateNewFrame(&frame, &props))) {
    return false;
  }
  PROPBAG2 option = {};
  option.pstrName = const_cast<LPOLESTR>(L"ImageQuality");
  VARIANT quality;
  VariantInit(&quality);
  quality.vt = VT_R4;
  quality.fltVal = 0.75f;
  props->Write(1, &option, &quality);
  WICPixelFormatGUID format = GUID_WICPixelFormat24bppBGR;
  if (FAILED(frame->Initialize(props.Get())) ||
      FAILED(frame->SetSize(w, h)) ||
      FAILED(frame->SetPixelFormat(&format)) ||
      FAILED(frame->WriteSource(converter.Get(), nullptr)) ||
      FAILED(frame->Commit()) || FAILED(encoder->Commit())) {
    return false;
  }
  STATSTG stat = {};
  if (FAILED(stream->Stat(&stat, STATFLAG_NONAME))) return false;
  out->resize(static_cast<size_t>(stat.cbSize.QuadPart));
  LARGE_INTEGER start = {};
  stream->Seek(start, STREAM_SEEK_SET, nullptr);
  ULONG read = 0;
  stream->Read(out->data(), static_cast<ULONG>(out->size()), &read);
  out->resize(read);
  return read > 0;
}

}  // namespace

// The largest picture read from a video: 8K (7680 by 4320 pixels), no side longer than
// 8192. Each picture is copied as width * height * 4 bytes, and a file can claim far
// more; it is not shown then.
constexpr UINT32 kMaxSide = 8192;
constexpr uint64_t kMaxPixels = uint64_t{7680} * 4320;

// Codecs of newer SDKs, by the FOURCC their subtype is made from (mfapi.h's rule).
static GUID FourccSubtype(DWORD fourcc) {
  GUID guid = MFVideoFormat_Base;
  guid.Data1 = fourcc;
  return guid;
}
// MFAudioFormat_Vorbis, as the Windows 10 SDK defines it.
static const GUID kVorbis = {
    0x8D2FD10B, 0x5841, 0x4a6b, {0x89, 0x05, 0x58, 0x8F, 0xEC, 0x1A, 0xDE, 0xD9}};

bool CodecAllowed(const GUID& major, const GUID& subtype) {
  if (major == MFMediaType_Video) {
    for (const GUID& known : {MFVideoFormat_VP80, MFVideoFormat_VP90,
                              FourccSubtype(MAKEFOURCC('A', 'V', '0', '1')),
                              MFVideoFormat_H264, MFVideoFormat_H264_ES,
                              MFVideoFormat_HEVC, MFVideoFormat_HEVC_ES,
                              MFVideoFormat_MP4V, MFVideoFormat_MP4S,
                              MFVideoFormat_M4S2, MFVideoFormat_MJPG}) {
      if (subtype == known) return true;
    }
    return false;
  }
  if (major == MFMediaType_Audio) {
    for (const GUID& known : {MFAudioFormat_AAC, MFAudioFormat_ADTS, MFAudioFormat_MP3,
                              MFAudioFormat_MPEG,
                              FourccSubtype(MAKEFOURCC('O', 'p', 'u', 's')), kVorbis,
                              MFAudioFormat_PCM, MFAudioFormat_Float}) {
      if (subtype == known) return true;
    }
    return false;
  }
  // Nothing else is decoded: no subtitles, no other kind of stream.
  return true;
}

bool CodecsAllowed(IMFSourceReader* reader) {
  bool any = false;
  for (DWORD stream = 0; stream < 64; stream++) {
    ComPtr<IMFMediaType> type;
    const HRESULT hr = reader->GetNativeMediaType(stream, 0, &type);
    if (hr == MF_E_INVALIDSTREAMNUMBER) break;
    if (FAILED(hr)) return false;
    GUID major = GUID_NULL;
    GUID subtype = GUID_NULL;
    if (FAILED(type->GetGUID(MF_MT_MAJOR_TYPE, &major)) ||
        FAILED(type->GetGUID(MF_MT_SUBTYPE, &subtype)) ||
        !CodecAllowed(major, subtype)) {
      return false;
    }
    any = true;
  }
  return any;
}

bool CodecsAllowed(const HeldVideo& video) {
  MediaThread media;
  ComPtr<IMFByteStream> stream;
  ComPtr<IMFSourceReader> reader;
  return video.NewStream(&stream) &&
         SUCCEEDED(MFCreateSourceReaderFromByteStream(stream.Get(), nullptr, &reader)) &&
         CodecsAllowed(reader.Get());
}

bool SizeAllowed(UINT32 width, UINT32 height) {
  return width > 0 && height > 0 && width <= kMaxSide && height <= kMaxSide &&
         uint64_t{width} * height <= kMaxPixels;
}

static Turns TurnsOf(IMFSourceReader* reader) {
  Turns out;
  ComPtr<IMFMediaType> native;
  if (FAILED(reader->GetNativeMediaType(
          static_cast<DWORD>(MF_SOURCE_READER_FIRST_VIDEO_STREAM), 0,
          &native))) {
    return out;
  }
  const UINT32 degrees =
      MFGetAttributeUINT32(native.Get(), MF_MT_VIDEO_ROTATION, 0);
  out.turns = static_cast<int>((degrees / 90) % 4);
  MFGetAttributeSize(native.Get(), MF_MT_FRAME_SIZE, &out.width, &out.height);
  return out;
}

Turns TurnsOf(IMFByteStream* stream) {
  ComPtr<IMFSourceReader> reader;
  if (stream == nullptr ||
      FAILED(MFCreateSourceReaderFromByteStream(stream, nullptr, &reader))) {
    return Turns();
  }
  return TurnsOf(reader.Get());
}

void Turn(std::vector<uint8_t>* pixels, UINT* width, UINT* height, int turns) {
  if (turns == 0) return;
  const size_t w = *width;
  const size_t h = *height;
  const auto* from = reinterpret_cast<const uint32_t*>(pixels->data());
  std::vector<uint8_t> turned(pixels->size());
  auto* to = reinterpret_cast<uint32_t*>(turned.data());
  const size_t tw = turns == 2 ? w : h;
  for (size_t y = 0; y < h; y++) {
    for (size_t x = 0; x < w; x++) {
      size_t nx = x;
      size_t ny = y;
      if (turns == 1) {
        nx = h - 1 - y;
        ny = x;
      } else if (turns == 2) {
        nx = w - 1 - x;
        ny = h - 1 - y;
      } else {
        nx = y;
        ny = w - 1 - x;
      }
      to[ny * tw + nx] = from[y * w + x];
    }
  }
  pixels->swap(turned);
  if (turns != 2) std::swap(*width, *height);
}


namespace {

// Hears the source resolver (opening the file) and the asynchronous source reader
// (choosing streams, decoding), and lets the probe wait for each answer until its time is
// up. Media Foundation holds it as long as it may still call it.
class Answers : public IMFSourceReaderCallback, public IMFAsyncCallback {
 public:
  using Clock = std::chrono::steady_clock;

  // IUnknown
  STDMETHODIMP QueryInterface(REFIID riid, void** object) override {
    if (riid == __uuidof(IUnknown) || riid == __uuidof(IMFSourceReaderCallback)) {
      *object = static_cast<IMFSourceReaderCallback*>(this);
    } else if (riid == __uuidof(IMFAsyncCallback)) {
      *object = static_cast<IMFAsyncCallback*>(this);
    } else {
      *object = nullptr;
      return E_NOINTERFACE;
    }
    AddRef();
    return S_OK;
  }
  STDMETHODIMP_(ULONG) AddRef() override { return ++refs_; }
  STDMETHODIMP_(ULONG) Release() override {
    const ULONG left = --refs_;
    if (left == 0) delete this;
    return left;
  }

  // IMFAsyncCallback: the source resolver is done (or gave up, once cancelled). A
  // source made once the probe gave up on it is shut down, on a thread of its own, away
  // from Media Foundation's call: nobody else would, and it would hold the file.
  STDMETHODIMP GetParameters(DWORD*, DWORD*) override { return E_NOTIMPL; }
  STDMETHODIMP Invoke(IMFAsyncResult* result) override {
    std::lock_guard<std::mutex> lock(mutex_);
    if (abandoned_) {
      try {
        std::thread([resolver = std::move(resolver_), result = ComPtr<IMFAsyncResult>(result),
                     stream = stream_] {
          MediaThread media;
          MF_OBJECT_TYPE type = MF_OBJECT_INVALID;
          ComPtr<IUnknown> made;
          const HRESULT ended =
              stream ? resolver->EndCreateObjectFromByteStream(result.Get(), &type, &made)
                     : resolver->EndCreateObjectFromURL(result.Get(), &type, &made);
          ComPtr<IMFMediaSource> source;
          if (SUCCEEDED(ended) && SUCCEEDED(made.As(&source))) source->Shutdown();
        }).detach();
      } catch (const std::system_error&) {
        // No thread to be had: the source stays with Media Foundation.
      }
      return S_OK;
    }
    resolved_ = result;
    changed_.notify_all();
    return S_OK;
  }

  // IMFSourceReaderCallback
  STDMETHODIMP OnReadSample(HRESULT status, DWORD, DWORD flags, LONGLONG,
                            IMFSample* sample) override {
    std::lock_guard<std::mutex> lock(mutex_);
    read_ = true;
    status_ = status;
    flags_ = flags;
    sample_ = sample;
    changed_.notify_all();
    return S_OK;
  }
  STDMETHODIMP OnFlush(DWORD) override {
    std::lock_guard<std::mutex> lock(mutex_);
    flushed_ = true;
    changed_.notify_all();
    return S_OK;
  }
  STDMETHODIMP OnEvent(DWORD, IMFMediaEvent*) override { return S_OK; }

  // Asks the resolver for the media source of the file at `path`, or of `stream` (which
  // `path` then names); false when it cannot start.
  bool Resolve(const std::wstring& path, IMFByteStream* stream,
               IMFSourceResolver* resolver) {
    const DWORD flags = MF_RESOLUTION_MEDIASOURCE | MF_RESOLUTION_READ;
    stream_ = stream != nullptr;
    resolver_ = resolver;
    return SUCCEEDED(
        stream == nullptr
            ? resolver->BeginCreateObjectFromURL(path.c_str(), flags, nullptr,
                                                 &cancel_, this, nullptr)
            : resolver->BeginCreateObjectFromByteStream(
                  stream, path.c_str(), flags, nullptr, &cancel_, this, nullptr));
  }

  // The media source, or nothing when it failed or did not come by `until`. Then the
  // opening is cancelled, and its answer waited for a little while: a source made
  // meanwhile is shut down, and so is one made later still (Invoke).
  ComPtr<IMFMediaSource> Resolved(IMFSourceResolver* resolver, Clock::time_point until) {
    ComPtr<IMFAsyncResult> result;
    bool late = false;
    {
      std::unique_lock<std::mutex> lock(mutex_);
      auto came = [this] { return resolved_ != nullptr; };
      if (!changed_.wait_until(lock, until, came)) {
        late = true;
        lock.unlock();
        resolver->CancelObjectCreation(cancel_.Get());
        lock.lock();
        if (!changed_.wait_for(lock, std::chrono::milliseconds(2000), came)) {
          // Invoke takes the resolver when the answer comes.
          abandoned_ = true;
          cancel_.Reset();
          return nullptr;
        }
      }
      // The answer, the cancel cookie (which holds this callback) and the resolver are
      // let go of here, or this would never be freed, nor what the answer holds.
      result = std::move(resolved_);
      cancel_.Reset();
      resolver_.Reset();
    }
    MF_OBJECT_TYPE type = MF_OBJECT_INVALID;
    ComPtr<IUnknown> made;
    ComPtr<IMFMediaSource> source;
    const HRESULT ended =
        stream_ ? resolver->EndCreateObjectFromByteStream(result.Get(), &type, &made)
                : resolver->EndCreateObjectFromURL(result.Get(), &type, &made);
    if (FAILED(ended) || FAILED(made.As(&source))) {
      return nullptr;
    }
    if (late) {
      source->Shutdown();
      return nullptr;
    }
    return source;
  }

  // Waits for the sample asked for: false when its time is up first.
  bool Read(Clock::time_point until, HRESULT* status, DWORD* flags,
            ComPtr<IMFSample>* sample) {
    std::unique_lock<std::mutex> lock(mutex_);
    if (!changed_.wait_until(lock, until, [this] { return read_; })) return false;
    read_ = false;
    *status = status_;
    *flags = flags_;
    *sample = std::move(sample_);
    return true;
  }

  // Waits for a flush to end, for at most `time`.
  void Flushed(std::chrono::milliseconds time) {
    std::unique_lock<std::mutex> lock(mutex_);
    changed_.wait_for(lock, time, [this] { return flushed_; });
  }

 private:
  std::atomic<ULONG> refs_{1};
  std::mutex mutex_;
  std::condition_variable changed_;
  ComPtr<IMFSourceResolver> resolver_;
  ComPtr<IUnknown> cancel_;
  ComPtr<IMFAsyncResult> resolved_;
  bool stream_ = false;
  bool abandoned_ = false;
  bool read_ = false;
  bool flushed_ = false;
  HRESULT status_ = S_OK;
  DWORD flags_ = 0;
  ComPtr<IMFSample> sample_;
};

// Shuts the media source down when the probe ends without a reader to do it.
struct SourceOwner {
  ComPtr<IMFMediaSource> source;
  ~SourceOwner() {
    if (source) source->Shutdown();
  }
};

// The asynchronous source reader, with a read pending or not: one still pending when the
// probe ends is cancelled (flushed) before the reader is let go of, and the reader shuts
// its media source down.
struct ReaderOwner {
  ComPtr<IMFSourceReader> reader;
  Answers* answers = nullptr;
  bool pending = false;
  ~ReaderOwner() {
    if (reader && pending) {
      reader->Flush(static_cast<DWORD>(MF_SOURCE_READER_ALL_STREAMS));
      answers->Flushed(std::chrono::milliseconds(2000));
    }
  }
};

bool ProbeAny(const std::wstring& path, IMFByteStream* stream, int max_width,
              Probe* out, std::chrono::milliseconds time) {
  const auto until = Answers::Clock::now() + time;
  MediaThread media;
  ComPtr<Answers> answers;
  answers.Attach(new Answers());
  ComPtr<IMFSourceResolver> resolver;
  if (FAILED(MFCreateSourceResolver(&resolver)) ||
      !answers->Resolve(path, stream, resolver.Get())) {
    return false;
  }
  SourceOwner source;
  source.source = answers->Resolved(resolver.Get(), until);
  if (!source.source) return false;
  ComPtr<IMFAttributes> attrs;
  ReaderOwner owner;
  owner.answers = answers.Get();
  if (FAILED(MFCreateAttributes(&attrs, 2)) ||
      FAILED(attrs->SetUINT32(MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, TRUE)) ||
      FAILED(attrs->SetUnknown(MF_SOURCE_READER_ASYNC_CALLBACK,
                               static_cast<IMFSourceReaderCallback*>(answers.Get()))) ||
      FAILED(MFCreateSourceReaderFromMediaSource(source.source.Get(), attrs.Get(),
                                                 &owner.reader))) {
    return false;
  }
  // The reader shuts the source down now.
  source.source.Reset();
  IMFSourceReader* reader = owner.reader.Get();
  // A codec the chat does not offer is not decoded at all, not even its first picture.
  if (!CodecsAllowed(reader)) return false;
  const DWORD video = static_cast<DWORD>(MF_SOURCE_READER_FIRST_VIDEO_STREAM);
  reader->SetStreamSelection(
      static_cast<DWORD>(MF_SOURCE_READER_ALL_STREAMS), FALSE);
  reader->SetStreamSelection(video, TRUE);
  ComPtr<IMFMediaType> wanted;
  if (FAILED(MFCreateMediaType(&wanted)) ||
      FAILED(wanted->SetGUID(MF_MT_MAJOR_TYPE, MFMediaType_Video)) ||
      FAILED(wanted->SetGUID(MF_MT_SUBTYPE, MFVideoFormat_RGB32)) ||
      FAILED(reader->SetCurrentMediaType(video, nullptr, wanted.Get()))) {
    return false;
  }
  ComPtr<IMFMediaType> current;
  UINT32 width = 0;
  UINT32 height = 0;
  if (FAILED(reader->GetCurrentMediaType(video, &current)) ||
      FAILED(MFGetAttributeSize(current.Get(), MF_MT_FRAME_SIZE, &width,
                                &height)) ||
      !SizeAllowed(width, height)) {
    return false;
  }
  // Rows go from the top when the stride is positive, from the bottom otherwise.
  const LONG stride = static_cast<LONG>(
      MFGetAttributeUINT32(current.Get(), MF_MT_DEFAULT_STRIDE, width * 4));
  ComPtr<IMFSample> sample;
  for (int i = 0; i < 200 && !sample; i++) {
    if (FAILED(reader->ReadSample(video, 0, nullptr, nullptr, nullptr, nullptr))) {
      return false;
    }
    owner.pending = true;
    HRESULT status = S_OK;
    DWORD flags = 0;
    if (!answers->Read(until, &status, &flags, &sample)) return false;
    owner.pending = false;
    if (FAILED(status) || (flags & MF_SOURCE_READERF_ENDOFSTREAM) ||
        (flags & MF_SOURCE_READERF_ERROR)) {
      return false;
    }
  }
  ComPtr<IMFMediaBuffer> buffer;
  if (!sample || FAILED(sample->ConvertToContiguousBuffer(&buffer))) {
    return false;
  }
  BYTE* data = nullptr;
  DWORD length = 0;
  if (FAILED(buffer->Lock(&data, nullptr, &length))) return false;
  // In 64 bits: the size and stride come from the file, and a negative stride must not
  // turn into a huge unsigned offset.
  const int64_t rows = height;
  const int64_t row = static_cast<int64_t>(width) * 4;
  const int64_t step = stride == 0 ? row : stride;
  const int64_t span = (step < 0 ? -step : step) * (rows - 1) + row;
  const bool fits = (step < 0 ? -step : step) >= row &&
                    span <= static_cast<int64_t>(length);
  std::vector<uint8_t> pixels(static_cast<size_t>(row * rows));
  if (fits) {
    const BYTE* first = step > 0 ? data : data + (rows - 1) * -step;
    for (int64_t y = 0; y < rows; y++) {
      memcpy(pixels.data() + y * row, first + y * step,
             static_cast<size_t>(row));
    }
  }
  buffer->Unlock();
  if (fits) Turn(&pixels, &width, &height, TurnsOf(reader).turns);
  if (!fits || !EncodeJpeg(pixels, width, height, max_width, &out->jpeg)) {
    return false;
  }
  out->width = static_cast<int>(width);
  out->height = static_cast<int>(height);
  PROPVARIANT duration;
  PropVariantInit(&duration);
  if (SUCCEEDED(reader->GetPresentationAttribute(
          static_cast<DWORD>(MF_SOURCE_READER_MEDIASOURCE), MF_PD_DURATION,
          &duration))) {
    out->duration_ms = static_cast<int64_t>(duration.uhVal.QuadPart / 10000);
  }
  PropVariantClear(&duration);
  return true;
}


}  // namespace

namespace {

// An IStream over a file handle of its own (a duplicate of the one checked), reading at a
// position of its own; Media Foundation makes a byte stream of it. Reads may come from
// Media Foundation's threads, so they take turns.
class HandleStream final : public IStream {
 public:
  HandleStream(HANDLE file, std::wstring name) : file_(file), name_(std::move(name)) {}

  HRESULT STDMETHODCALLTYPE QueryInterface(REFIID riid, void** object) override {
    if (object == nullptr) return E_POINTER;
    if (riid == __uuidof(IUnknown) || riid == __uuidof(IStream) ||
        riid == __uuidof(ISequentialStream)) {
      *object = static_cast<IStream*>(this);
      AddRef();
      return S_OK;
    }
    *object = nullptr;
    return E_NOINTERFACE;
  }
  ULONG STDMETHODCALLTYPE AddRef() override { return ++refs_; }
  ULONG STDMETHODCALLTYPE Release() override {
    const ULONG left = --refs_;
    if (left == 0) delete this;
    return left;
  }

  HRESULT STDMETHODCALLTYPE Read(void* into, ULONG size, ULONG* read) override {
    std::lock_guard<std::mutex> lock(mutex_);
    OVERLAPPED at = {};
    at.Offset = static_cast<DWORD>(position_ & 0xFFFFFFFF);
    at.OffsetHigh = static_cast<DWORD>(position_ >> 32);
    DWORD got = 0;
    if (!ReadFile(file_, into, size, &got, &at) &&
        GetLastError() != ERROR_HANDLE_EOF) {
      if (read != nullptr) *read = 0;
      return HRESULT_FROM_WIN32(GetLastError());
    }
    position_ += got;
    if (read != nullptr) *read = got;
    return got < size ? S_FALSE : S_OK;
  }
  HRESULT STDMETHODCALLTYPE Write(const void*, ULONG, ULONG*) override {
    return STG_E_ACCESSDENIED;
  }
  HRESULT STDMETHODCALLTYPE Seek(LARGE_INTEGER move, DWORD origin,
                                 ULARGE_INTEGER* now) override {
    std::lock_guard<std::mutex> lock(mutex_);
    int64_t base = 0;
    if (origin == STREAM_SEEK_CUR) {
      base = static_cast<int64_t>(position_);
    } else if (origin == STREAM_SEEK_END) {
      LARGE_INTEGER size;
      if (!GetFileSizeEx(file_, &size)) return HRESULT_FROM_WIN32(GetLastError());
      base = size.QuadPart;
    } else if (origin != STREAM_SEEK_SET) {
      return STG_E_INVALIDFUNCTION;
    }
    const int64_t to = base + move.QuadPart;
    if (to < 0) return STG_E_INVALIDFUNCTION;
    position_ = static_cast<uint64_t>(to);
    if (now != nullptr) now->QuadPart = position_;
    return S_OK;
  }
  HRESULT STDMETHODCALLTYPE SetSize(ULARGE_INTEGER) override { return STG_E_ACCESSDENIED; }
  HRESULT STDMETHODCALLTYPE CopyTo(IStream*, ULARGE_INTEGER, ULARGE_INTEGER*,
                                   ULARGE_INTEGER*) override {
    return E_NOTIMPL;
  }
  HRESULT STDMETHODCALLTYPE Commit(DWORD) override { return S_OK; }
  HRESULT STDMETHODCALLTYPE Revert() override { return E_NOTIMPL; }
  HRESULT STDMETHODCALLTYPE LockRegion(ULARGE_INTEGER, ULARGE_INTEGER, DWORD) override {
    return STG_E_INVALIDFUNCTION;
  }
  HRESULT STDMETHODCALLTYPE UnlockRegion(ULARGE_INTEGER, ULARGE_INTEGER, DWORD) override {
    return STG_E_INVALIDFUNCTION;
  }
  HRESULT STDMETHODCALLTYPE Stat(STATSTG* stat, DWORD flags) override {
    if (stat == nullptr) return E_POINTER;
    *stat = {};
    LARGE_INTEGER size;
    if (!GetFileSizeEx(file_, &size)) return HRESULT_FROM_WIN32(GetLastError());
    stat->type = STGTY_STREAM;
    stat->cbSize.QuadPart = static_cast<ULONGLONG>(size.QuadPart);
    stat->grfMode = STGM_READ | STGM_SHARE_DENY_NONE;
    if ((flags & STATFLAG_NONAME) == 0) {
      const size_t bytes = (name_.size() + 1) * sizeof(wchar_t);
      stat->pwcsName = static_cast<LPOLESTR>(CoTaskMemAlloc(bytes));
      if (stat->pwcsName == nullptr) return E_OUTOFMEMORY;
      std::memcpy(stat->pwcsName, name_.c_str(), bytes);
    }
    return S_OK;
  }
  HRESULT STDMETHODCALLTYPE Clone(IStream**) override { return E_NOTIMPL; }

 private:
  ~HandleStream() { CloseHandle(file_); }

  std::atomic<ULONG> refs_{1};
  std::mutex mutex_;
  const HANDLE file_;
  const std::wstring name_;
  uint64_t position_ = 0;
};

}  // namespace

HeldVideo::~HeldVideo() {
  if (file_ != INVALID_HANDLE_VALUE) CloseHandle(file_);
}

bool HeldVideo::Hold(const std::wstring& path, const std::vector<Head>& heads) {
  if (file_ != INVALID_HANDLE_VALUE) return false;
  // Shared every way, as a player opens a file: the user's own applications may still
  // write, move or delete it. Without FILE_FLAG_BACKUP_SEMANTICS a folder does not open.
  HANDLE file = CreateFileW(path.c_str(), GENERIC_READ,
                            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                            nullptr, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, nullptr);
  if (file == INVALID_HANDLE_VALUE) return false;
  uint8_t head[kHeadBytes] = {};
  DWORD read = 0;
  OVERLAPPED start = {};
  if (GetFileType(file) != FILE_TYPE_DISK ||
      !ReadFile(file, head, static_cast<DWORD>(sizeof head), &read, &start) ||
      !HeadMatches(heads, head, read)) {
    CloseHandle(file);
    return false;
  }
  file_ = file;
  name_ = path;
  return true;
}

bool HeldVideo::NewStream(IMFByteStream** out) const {
  *out = nullptr;
  if (file_ == INVALID_HANDLE_VALUE) return false;
  HANDLE own = nullptr;
  if (!DuplicateHandle(GetCurrentProcess(), file_, GetCurrentProcess(), &own, 0, FALSE,
                       DUPLICATE_SAME_ACCESS)) {
    return false;
  }
  ComPtr<IStream> stream;
  stream.Attach(new HandleStream(own, name_));
  return SUCCEEDED(MFCreateMFByteStreamOnStream(stream.Get(), out));
}

bool ProbeHeld(const HeldVideo& video, int max_width, Probe* out,
               std::chrono::milliseconds time) {
  ComPtr<IMFByteStream> stream;
  return video.NewStream(&stream) &&
         ProbeVideoStream(stream.Get(), video.name(), max_width, out, time);
}

bool ProbeVideo(const std::wstring& path, int max_width, Probe* out,
                std::chrono::milliseconds time) {
  return ProbeAny(path, nullptr, max_width, out, time);
}

bool ProbeVideoStream(IMFByteStream* stream, const std::wstring& name,
                      int max_width, Probe* out, std::chrono::milliseconds time) {
  return ProbeAny(name, stream, max_width, out, time);
}

}  // namespace fogata_video
