#include "flutter_window.h"

#include <flutter/method_channel.h>
#include <flutter/standard_method_codec.h>

#include <optional>
#include <string>
#include <variant>

#include "flutter/generated_plugin_registrant.h"

namespace {

// Opening Fogata when the person signs in (see lib/src/autostart.dart): a value under the
// user's own Run key, which needs no administrator.
constexpr wchar_t kRunKey[] = L"Software\\Microsoft\\Windows\\CurrentVersion\\Run";
constexpr wchar_t kRunValue[] = L"Fogata";

// This executable's path in quotes, as the Run key wants a path that may have spaces;
// empty if Windows does not say it.
std::wstring QuotedExecutable() {
  std::wstring path(MAX_PATH, L'\0');
  for (;;) {
    const DWORD length = GetModuleFileNameW(nullptr, path.data(),
                                            static_cast<DWORD>(path.size()));
    if (length == 0) {
      return L"";
    }
    if (length < path.size()) {
      path.resize(length);
      break;
    }
    path.resize(path.size() * 2);
  }
  return L"\"" + path + L"\"";
}

bool AutostartOn() {
  DWORD size = 0;
  return RegGetValueW(HKEY_CURRENT_USER, kRunKey, kRunValue, RRF_RT_REG_SZ,
                      nullptr, nullptr, &size) == ERROR_SUCCESS;
}

LSTATUS SetAutostart(bool on) {
  if (!on) {
    const LSTATUS status =
        RegDeleteKeyValueW(HKEY_CURRENT_USER, kRunKey, kRunValue);
    return status == ERROR_FILE_NOT_FOUND ? ERROR_SUCCESS : status;
  }
  const std::wstring command = QuotedExecutable();
  if (command.empty()) {
    return ERROR_PATH_NOT_FOUND;
  }
  return RegSetKeyValueW(
      HKEY_CURRENT_USER, kRunKey, kRunValue, REG_SZ, command.c_str(),
      static_cast<DWORD>((command.size() + 1) * sizeof(wchar_t)));
}

}  // namespace

FlutterWindow::FlutterWindow(const flutter::DartProject& project)
    : project_(project) {}

FlutterWindow::~FlutterWindow() {}

bool FlutterWindow::OnCreate() {
  if (!Win32Window::OnCreate()) {
    return false;
  }

  RECT frame = GetClientArea();

  // The size here must match the window dimensions to avoid unnecessary surface
  // creation / destruction in the startup path.
  flutter_controller_ = std::make_unique<flutter::FlutterViewController>(
      frame.right - frame.left, frame.bottom - frame.top, project_);
  // Ensure that basic setup of the controller was successful.
  if (!flutter_controller_->engine() || !flutter_controller_->view()) {
    return false;
  }
  RegisterPlugins(flutter_controller_->engine());

  autostart_channel_ =
      std::make_unique<flutter::MethodChannel<flutter::EncodableValue>>(
          flutter_controller_->engine()->messenger(), "fogata/autostart",
          &flutter::StandardMethodCodec::GetInstance());
  autostart_channel_->SetMethodCallHandler(
      [](const flutter::MethodCall<flutter::EncodableValue>& call,
         std::unique_ptr<flutter::MethodResult<flutter::EncodableValue>>
             result) {
        if (call.method_name() == "get") {
          result->Success(
              flutter::EncodableValue(std::string(AutostartOn() ? "on" : "off")));
        } else if (call.method_name() == "set") {
          const bool* on = std::get_if<bool>(call.arguments());
          const LSTATUS status = SetAutostart(on != nullptr && *on);
          if (status != ERROR_SUCCESS) {
            result->Error("set", "The registry refused the change (error " +
                                     std::to_string(status) + ").");
            return;
          }
          result->Success(
              flutter::EncodableValue(std::string(AutostartOn() ? "on" : "off")));
        } else {
          result->NotImplemented();
        }
      });
  SetChildContent(flutter_controller_->view()->GetNativeWindow());

  flutter_controller_->engine()->SetNextFrameCallback([&]() {
    this->Show();
  });

  // Flutter can complete the first frame before the "show window" callback is
  // registered. The following call ensures a frame is pending to ensure the
  // window is shown. It is a no-op if the first frame hasn't completed yet.
  flutter_controller_->ForceRedraw();

  return true;
}

void FlutterWindow::OnDestroy() {
  autostart_channel_ = nullptr;
  if (flutter_controller_) {
    flutter_controller_ = nullptr;
  }

  Win32Window::OnDestroy();
}

LRESULT
FlutterWindow::MessageHandler(HWND hwnd, UINT const message,
                              WPARAM const wparam,
                              LPARAM const lparam) noexcept {
  // Give Flutter, including plugins, an opportunity to handle window messages.
  if (flutter_controller_) {
    std::optional<LRESULT> result =
        flutter_controller_->HandleTopLevelWindowProc(hwnd, message, wparam,
                                                      lparam);
    if (result) {
      return *result;
    }
  }

  switch (message) {
    case WM_FONTCHANGE:
      flutter_controller_->engine()->ReloadSystemFonts();
      break;
  }

  return Win32Window::MessageHandler(hwnd, message, wparam, lparam);
}
