#include "include/fogata_video/fogata_video_plugin_c_api.h"

#include <flutter/plugin_registrar_windows.h>

#include "fogata_video_plugin.h"

void FogataVideoPluginCApiRegisterWithRegistrar(
    FlutterDesktopPluginRegistrarRef registrar) {
  fogata_video::FogataVideoPlugin::RegisterWithRegistrar(
      flutter::PluginRegistrarManager::GetInstance()
          ->GetRegistrar<flutter::PluginRegistrarWindows>(registrar));
}
