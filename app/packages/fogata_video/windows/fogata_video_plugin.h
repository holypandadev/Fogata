#ifndef FLUTTER_PLUGIN_FOGATA_VIDEO_PLUGIN_H_
#define FLUTTER_PLUGIN_FOGATA_VIDEO_PLUGIN_H_

#include <windows.h>

#include <flutter/method_channel.h>
#include <flutter/plugin_registrar_windows.h>

#include <cstdint>
#include <map>
#include <memory>

namespace fogata_video {

class Player;
struct Probes;

// Videos on Windows with Media Foundation: a video's first picture and length, and players
// that draw into Flutter textures.
class FogataVideoPlugin : public flutter::Plugin {
 public:
  static void RegisterWithRegistrar(flutter::PluginRegistrarWindows* registrar);

  explicit FogataVideoPlugin(flutter::PluginRegistrarWindows* registrar);
  ~FogataVideoPlugin() override;

  FogataVideoPlugin(const FogataVideoPlugin&) = delete;
  FogataVideoPlugin& operator=(const FogataVideoPlugin&) = delete;

 private:
  void HandleMethodCall(
      const flutter::MethodCall<flutter::EncodableValue>& call,
      std::unique_ptr<flutter::MethodResult<flutter::EncodableValue>> result);

  // Answers a probe that finished away from the platform thread.
  void AnswerProbe(int job);

  flutter::PluginRegistrarWindows* registrar_;
  flutter::TextureRegistrar* textures_;
  int window_proc_ = -1;
  std::map<int64_t, Player*> players_;

  // Probes running, shared with their threads, which may end after the plugin.
  std::shared_ptr<Probes> probes_;
};

}  // namespace fogata_video

#endif  // FLUTTER_PLUGIN_FOGATA_VIDEO_PLUGIN_H_
