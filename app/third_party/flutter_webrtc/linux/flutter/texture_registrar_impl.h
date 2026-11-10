// Copyright 2013 The Flutter Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#ifndef FLUTTER_SHELL_PLATFORM_COMMON_CLIENT_WRAPPER_TEXTURE_REGISTRAR_IMPL_H_
#define FLUTTER_SHELL_PLATFORM_COMMON_CLIENT_WRAPPER_TEXTURE_REGISTRAR_IMPL_H_

#include <map>
#include <mutex>

#include "include/flutter/texture_registrar.h"

struct FlTextureProxy;

namespace flutter {

// Wrapper around a FlTextureRegistrar that implements the
// TextureRegistrar API.
class TextureRegistrarImpl : public TextureRegistrar {
 public:
  explicit TextureRegistrarImpl(FlTextureRegistrar* texture_registrar_ref);
  virtual ~TextureRegistrarImpl();

  // Prevent copying.
  TextureRegistrarImpl(TextureRegistrarImpl const&) = delete;
  TextureRegistrarImpl& operator=(TextureRegistrarImpl const&) = delete;

  // |flutter::TextureRegistrar|
  int64_t RegisterTexture(TextureVariant* texture) override;

  // |flutter::TextureRegistrar|
  bool MarkTextureFrameAvailable(int64_t texture_id) override;

  // |flutter::TextureRegistrar|
  bool UnregisterTexture(int64_t texture_id) override;

  // |flutter::TextureRegistrar|
  bool UnregisterTextureLater(int64_t texture_id,
                              std::function<void()> release) override;

 private:
  // Takes a texture out of the engine and of [textures_]; its proxy, still
  // referenced, or null.
  FlTextureProxy* Unregister(int64_t texture_id, bool* unregistered);

  // Handle for interacting with the C API.
  FlTextureRegistrar* texture_registrar_ref_;
  // Fogata: frames are marked available on the threads they arrive on, while the
  // platform thread registers and unregisters.
  std::mutex mutex_;
  std::map<int64_t, FlTextureProxy*> textures_;
};

}  // namespace flutter

#endif  // FLUTTER_SHELL_PLATFORM_COMMON_CLIENT_WRAPPER_TEXTURE_REGISTRAR_IMPL_H_
