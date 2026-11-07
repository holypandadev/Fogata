#ifndef FOGATA_DENOISE_TABLE_H
#define FOGATA_DENOISE_TABLE_H

// Fogata: the table of noise suppression functions the application hands over by its
// address (see fogata_denoise.h), taken only if it is one.

#include <cstdint>

#ifdef _WIN32
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>
#else
#include <dlfcn.h>
#endif

#include "fogata_denoise.h"

namespace flutter_webrtc_plugin {

// The library loaded in this process that |address| lies in; null outside every one
// (the heap, a stack, nothing at all).
inline const void* FogataImageOf(const void* address) {
#ifdef _WIN32
  HMODULE module = nullptr;
  if (!GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS |
                              GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                          static_cast<LPCWSTR>(address), &module)) {
    return nullptr;
  }
  return module;
#else
  Dl_info info;
  if (dladdr(address, &info) == 0) return nullptr;
  return info.dli_fbase;
#endif
}

// The table at |address| if it is one: inside a library loaded in this process, with the
// version this code knows, and both its functions in that same library. Any other address
// (a mistake, or a value the application should never have had) is not read past finding
// where it lies, and nothing in it is called.
inline const FogataDenoiseApi* FogataDenoiseTable(int64_t address) {
  if (address <= 0) return nullptr;
  const auto* table =
      reinterpret_cast<const FogataDenoiseApi*>(static_cast<intptr_t>(address));
  const void* image = FogataImageOf(table);
  if (image == nullptr || table->version != kFogataDenoiseApiVersion ||
      table->create == nullptr || table->process == nullptr ||
      FogataImageOf(reinterpret_cast<const void*>(table->create)) != image ||
      FogataImageOf(reinterpret_cast<const void*>(table->process)) != image) {
    return nullptr;
  }
  return table;
}

}  // namespace flutter_webrtc_plugin

#endif  // FOGATA_DENOISE_TABLE_H
