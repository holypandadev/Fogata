#ifndef FOGATA_DEFAULT_DEVICE_H_
#define FOGATA_DEFAULT_DEVICE_H_

// Fogata: the system's default microphone and speaker in libwebrtc's lists of devices.
//
// The application takes the first device listed for the system's default (call.dart's
// `_first`, mic_test.dart's `_choose`), and getUserMedia without a device records from
// the first input. On Linux that is right: libwebrtc's PulseAudio device lists the
// default first. On Windows it lists the active endpoints in the order Windows enumerates
// them, with no default among them, so the first may be any device (a virtual cable's
// 16-channel input, a phone's virtual microphone): a call that chose "the system's
// default" by its id played to, and recorded from, that device. Windows' default for calls
// (the communications role, which libwebrtc's voice engine itself starts with) is found
// here by its endpoint id, the same id the list gives as each device's guid, and listed
// first. Elsewhere nothing is known here and nothing changes. The order is plain C++
// (test/default_device_test.cc); only the endpoint's id needs Windows.

#include <cstdint>
#include <string>
#include <vector>

#ifdef _WIN32
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>

#include <mmdeviceapi.h>
#include <objbase.h>
#endif

namespace flutter_webrtc_plugin {

// The place of the device whose guid is `wanted` in a list of devices' guids, in the
// list's order; 0, the first as before, when it is not there or not known ("").
inline uint16_t DefaultDevicePlace(const std::vector<std::string>& guids,
                                   const std::string& wanted) {
  if (wanted.empty())
    return 0;
  for (size_t i = 0; i < guids.size(); i++) {
    if (guids[i] == wanted)
      return static_cast<uint16_t>(i);
  }
  return 0;
}

// The order to list `count` devices in: the one at `first` first, then the others in
// their own order. Each device's place in libwebrtc's list stays as it is: a device is
// selected by its id, looked up in that list.
inline std::vector<uint16_t> ListingOrder(int count, uint16_t first) {
  std::vector<uint16_t> order;
  if (count <= 0)
    return order;
  if (first >= count)
    first = 0;
  order.push_back(first);
  for (int i = 0; i < count; i++) {
    if (i != first)
      order.push_back(static_cast<uint16_t>(i));
  }
  return order;
}

// The endpoint id of the system's default input (`capture`) or output for calls, as
// libwebrtc gives it for a device's guid; "" where it cannot be told (not Windows, COM
// not usable on this thread, no such device). Called on the platform thread, where the
// application's runner has started COM.
inline std::string DefaultAudioEndpoint(bool capture) {
#ifdef _WIN32
  IMMDeviceEnumerator* enumerator = nullptr;
  if (FAILED(CoCreateInstance(__uuidof(MMDeviceEnumerator), nullptr, CLSCTX_ALL,
                              __uuidof(IMMDeviceEnumerator),
                              reinterpret_cast<void**>(&enumerator))) ||
      enumerator == nullptr) {
    return std::string();
  }
  IMMDevice* device = nullptr;
  HRESULT hr = enumerator->GetDefaultAudioEndpoint(
      capture ? eCapture : eRender, eCommunications, &device);
  enumerator->Release();
  if (FAILED(hr) || device == nullptr)
    return std::string();
  LPWSTR wide = nullptr;
  hr = device->GetId(&wide);
  device->Release();
  if (FAILED(hr) || wide == nullptr)
    return std::string();
  // UTF-8, as libwebrtc turns the same id into a device's guid.
  std::string id;
  const int size =
      WideCharToMultiByte(CP_UTF8, 0, wide, -1, nullptr, 0, nullptr, nullptr);
  if (size > 1) {
    id.resize(static_cast<size_t>(size));
    WideCharToMultiByte(CP_UTF8, 0, wide, -1, &id[0], size, nullptr, nullptr);
    id.resize(static_cast<size_t>(size - 1));
  }
  CoTaskMemFree(wide);
  return id;
#else
  (void)capture;
  return std::string();
#endif
}

}  // namespace flutter_webrtc_plugin

#endif  // FOGATA_DEFAULT_DEVICE_H_
