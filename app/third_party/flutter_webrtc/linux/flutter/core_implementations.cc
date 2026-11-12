// Copyright 2013 The Flutter Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

// This file contains the implementations of any class in the wrapper that
// - is not fully inline, and
// - is necessary for all clients of the wrapper (either app or plugin).
// It exists instead of the usual structure of having some_class_name.cc files
// so that changes to the set of things that need non-header implementations
// are not breaking changes for the template.
//
// If https://github.com/flutter/flutter/issues/57146 is fixed, this can be
// removed in favor of the normal structure since templates will no longer
// manually include files.

#include <atomic>
#include <cassert>
#include <iostream>
#include <mutex>
#include <utility>
#include <variant>
#include <vector>

#include "binary_messenger_impl.h"
#include "include/flutter/engine_method_result.h"
#include "include/flutter/texture_registrar.h"
#include "retired_textures.h"
#include "texture_registrar_impl.h"

struct FlTextureProxy {
  FlPixelBufferTexture parent_instance;
  // Fogata: read on the raster thread while the platform thread clears it. GObject
  // zeroes the instance (no constructor runs), which is a null pointer.
  std::atomic<flutter::TextureVariant*> texture;
};

struct FlTextureProxyClass {
  FlPixelBufferTextureClass parent_class;
};

G_DEFINE_TYPE(FlTextureProxy,
              fl_texture_proxy,
              fl_pixel_buffer_texture_get_type())

#define FL_TEXTURE_PROXY(obj)                                     \
  (G_TYPE_CHECK_INSTANCE_CAST((obj), fl_texture_proxy_get_type(), \
                              FlTextureProxy))

// Fogata: what a texture shows before its first frame, one transparent pixel. Given no
// buffer, the engine makes a texture of the size drawn without filling it, and the
// picture shows whatever that memory held (pieces of other pictures).
static const uint8_t kNoFrame[4] = {0, 0, 0, 0};

static gboolean fl_texture_proxy_no_frame(const uint8_t** out_buffer,
                                          uint32_t* width,
                                          uint32_t* height) {
  *out_buffer = kNoFrame;
  *width = 1;
  *height = 1;
  return TRUE;
}

// Fogata: unregistered textures, kept until the engine is surely done with them. The
// engine looks a texture up and copies its pixels on its raster thread without holding
// a reference, and uses the pixels after copy_pixels returns, so a texture unregistered
// on the platform thread may still be in use there, and with it the renderer whose
// buffer it reads (freed at once, a participant leaving as a frame of theirs was being
// drawn read freed memory). That thread copies one texture at a time: once it starts
// copying another texture after the unregistering, the old one is no longer in use and
// never will be. The same rule as packages/fogata_video's `retire`. What the texture
// showed is also let go of a few seconds after the unregistering if the engine copies
// no other texture meanwhile (see RetiredTextures::Expire).
namespace {

flutter::RetiredTextures<FlTextureProxy*> retired;

// How long after its unregistering what a texture showed goes at the latest.
constexpr guint kExpireSeconds = 5;

gboolean run_released(gpointer) {
  for (auto& release : retired.TakeReleased()) release();
  return G_SOURCE_REMOVE;
}

gboolean expire(gpointer serial) {
  if (auto release = retired.Expire(uint64_t(GPOINTER_TO_SIZE(serial)))) release();
  return G_SOURCE_REMOVE;
}

// Hands the plugin's reference to [proxy] over to the list above. On the platform
// thread, whose main context runs the timeout.
void retire(FlTextureProxy* proxy, std::function<void()> release) {
  const uint64_t serial = retired.Retire(proxy, std::move(release));
  g_timeout_add_seconds(kExpireSeconds, expire, GSIZE_TO_POINTER(gsize(serial)));
}

// On the raster thread, as [current] is about to be copied: the textures it has moved
// past go. Their last unref runs here, on the thread whose GL context made their GL
// textures; what they read is freed on the platform thread.
void moved_on(FlTextureProxy* current) {
  bool release = false;
  for (FlTextureProxy* gone : retired.MovedOn(current, &release)) g_object_unref(gone);
  if (release) g_idle_add(run_released, nullptr);
}

}  // namespace

static gboolean fl_texture_proxy_copy_pixels(FlPixelBufferTexture* texture,
                                             const uint8_t** out_buffer,
                                             uint32_t* width,
                                             uint32_t* height,
                                             GError** error) {
  FlTextureProxy* proxy = FL_TEXTURE_PROXY(texture);
  moved_on(proxy);
  flutter::TextureVariant* variant = proxy->texture.load();
  if (variant == nullptr) {
    return fl_texture_proxy_no_frame(out_buffer, width, height);
  }
  flutter::PixelBufferTexture& pixel_buffer =
      std::get<flutter::PixelBufferTexture>(*variant);
  const FlutterDesktopPixelBuffer* copy =
      pixel_buffer.CopyPixelBuffer(*width, *height);
  if (copy == nullptr) {
    return fl_texture_proxy_no_frame(out_buffer, width, height);
  }
  *out_buffer = copy->buffer;
  *width = copy->width;
  *height = copy->height;
  return TRUE;
}

static FlTextureProxy* fl_texture_proxy_new(flutter::TextureVariant* texture) {
  FlTextureProxy* proxy =
      FL_TEXTURE_PROXY(g_object_new(fl_texture_proxy_get_type(), nullptr));
  proxy->texture.store(texture);
  return proxy;
}

static void fl_texture_proxy_class_init(FlTextureProxyClass* klass) {
  FL_PIXEL_BUFFER_TEXTURE_CLASS(klass)->copy_pixels =
      fl_texture_proxy_copy_pixels;
}

static void fl_texture_proxy_init(FlTextureProxy* self) {}

namespace flutter {

// ========== binary_messenger_impl.h ==========

namespace {
// Passes |message| to |user_data|, which must be a BinaryMessageHandler, along
// with a BinaryReply that will send a response on |message|'s response handle.
//
// This serves as an adaptor between the function-pointer-based message callback
// interface provided by the C API and the std::function-based message handler
// interface of BinaryMessenger.
static void ForwardToHandler(FlBinaryMessenger* messenger,
                             const gchar* channel,
                             GBytes* message,
                             FlBinaryMessengerResponseHandle* response_handle,
                             gpointer user_data) {
  auto handler = g_object_ref(response_handle);
  BinaryReply reply_handler = [messenger, handler](const uint8_t* reply,
                                                   size_t reply_size) mutable {
    if (!handler) {
      std::cerr << "Error: Response can be set only once. Ignoring "
                   "duplicate response."
                << std::endl;
      return;
    }

    g_autoptr(GBytes) response = g_bytes_new(reply, reply_size);
    GError* error = nullptr;
    if (!fl_binary_messenger_send_response(
            messenger, (FlBinaryMessengerResponseHandle*)handler, response,
            &error)) {
      g_warning("Failed to send binary response: %s", error->message);
    }
  };

  const BinaryMessageHandler& message_handler =
      *static_cast<BinaryMessageHandler*>(user_data);

  if (user_data == nullptr) {
    std::cerr << "Error: user_data is null" << std::endl;
    return;
  }

  message_handler(
      static_cast<const uint8_t*>(g_bytes_get_data(message, nullptr)),
      g_bytes_get_size(message), std::move(reply_handler));
}
}  // namespace

BinaryMessengerImpl::BinaryMessengerImpl(FlBinaryMessenger* core_messenger)
    : messenger_(core_messenger) {}

BinaryMessengerImpl::~BinaryMessengerImpl() = default;

struct Captures {
  BinaryReply reply;
};

static void message_reply_cb(GObject* object,
                             GAsyncResult* result,
                             gpointer user_data) {
  g_autoptr(GError) error = nullptr;
  auto captures = reinterpret_cast<Captures*>(user_data);
  g_autoptr(GBytes) message = fl_binary_messenger_send_on_channel_finish(
      FL_BINARY_MESSENGER(object), result, &error);
  captures->reply(
      static_cast<const uint8_t*>(g_bytes_get_data(message, nullptr)),
      g_bytes_get_size(message));
  delete captures;
};

void BinaryMessengerImpl::Send(const std::string& channel,
                               const uint8_t* message,
                               size_t message_size,
                               BinaryReply reply) const {
  if (reply == nullptr) {
    g_autoptr(GBytes) data = g_bytes_new(message, message_size);
    fl_binary_messenger_send_on_channel(messenger_, channel.c_str(), data,
                                        nullptr, nullptr, nullptr);
    return;
  }

  auto captures = new Captures();
  captures->reply = reply;

  g_autoptr(GBytes) data = g_bytes_new(message, message_size);
  fl_binary_messenger_send_on_channel(messenger_, channel.c_str(), data,
                                      nullptr, message_reply_cb, captures);
}

void BinaryMessengerImpl::SetMessageHandler(const std::string& channel,
                                            BinaryMessageHandler handler) {
  if (!handler) {
    handlers_.erase(channel);
    fl_binary_messenger_set_message_handler_on_channel(
        messenger_, channel.c_str(), nullptr, nullptr, nullptr);
    return;
  }
  // Save the handler, to keep it alive.
  handlers_[channel] = std::move(handler);
  BinaryMessageHandler* message_handler = &handlers_[channel];
  // Set an adaptor callback that will invoke the handler.
  fl_binary_messenger_set_message_handler_on_channel(
      messenger_, channel.c_str(), ForwardToHandler, message_handler, nullptr);
}

// ========== engine_method_result.h ==========

namespace internal {

ReplyManager::ReplyManager(BinaryReply reply_handler)
    : reply_handler_(std::move(reply_handler)) {
  assert(reply_handler_);
}

ReplyManager::~ReplyManager() {
  if (reply_handler_) {
    // Warn, rather than send a not-implemented response, since the engine may
    // no longer be valid at this point.
    std::cerr
        << "Warning: Failed to respond to a message. This is a memory leak."
        << std::endl;
  }
}

void ReplyManager::SendResponseData(const std::vector<uint8_t>* data) {
  if (!reply_handler_) {
    std::cerr
        << "Error: Only one of Success, Error, or NotImplemented can be "
           "called,"
        << " and it can be called exactly once. Ignoring duplicate result."
        << std::endl;
    return;
  }

  const uint8_t* message = data && !data->empty() ? data->data() : nullptr;
  size_t message_size = data ? data->size() : 0;
  reply_handler_(message, message_size);
  reply_handler_ = nullptr;
}

}  // namespace internal

// ========== texture_registrar_impl.h ==========

TextureRegistrarImpl::TextureRegistrarImpl(
    FlTextureRegistrar* texture_registrar_ref)
    : texture_registrar_ref_(texture_registrar_ref) {}

TextureRegistrarImpl::~TextureRegistrarImpl() = default;

int64_t TextureRegistrarImpl::RegisterTexture(TextureVariant* texture) {
  auto texture_proxy = fl_texture_proxy_new(texture);
  fl_texture_registrar_register_texture(texture_registrar_ref_,
                                        FL_TEXTURE(texture_proxy));
  int64_t texture_id = reinterpret_cast<int64_t>(texture_proxy);
  std::lock_guard<std::mutex> lock(mutex_);
  textures_[texture_id] = texture_proxy;
  return texture_id;
}

bool TextureRegistrarImpl::MarkTextureFrameAvailable(int64_t texture_id) {
  std::lock_guard<std::mutex> lock(mutex_);
  auto it = textures_.find(texture_id);
  if (it != textures_.end()) {
    return fl_texture_registrar_mark_texture_frame_available(
        texture_registrar_ref_, FL_TEXTURE(it->second));
  }
  return false;
}

FlTextureProxy* TextureRegistrarImpl::Unregister(int64_t texture_id,
                                                 bool* unregistered) {
  FlTextureProxy* texture = nullptr;
  {
    std::lock_guard<std::mutex> lock(mutex_);
    auto it = textures_.find(texture_id);
    if (it == textures_.end()) return nullptr;
    texture = it->second;
    textures_.erase(it);
  }
  texture->texture.store(nullptr);
  *unregistered = fl_texture_registrar_unregister_texture(
      texture_registrar_ref_, FL_TEXTURE(texture));
  return texture;
}

bool TextureRegistrarImpl::UnregisterTexture(int64_t texture_id) {
  bool success = false;
  FlTextureProxy* texture = Unregister(texture_id, &success);
  if (texture == nullptr) return false;
  g_object_unref(texture);
  return success;
}

bool TextureRegistrarImpl::UnregisterTextureLater(
    int64_t texture_id,
    std::function<void()> release) {
  bool success = false;
  FlTextureProxy* texture = Unregister(texture_id, &success);
  if (texture == nullptr) {
    release();
    return false;
  }
  // The engine may still be copying its pixels on its own thread: the texture, and
  // what it reads, go once it has moved on.
  retire(texture, std::move(release));
  return success;
}

}  // namespace flutter
