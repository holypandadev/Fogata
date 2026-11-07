import Cocoa
import FlutterMacOS
import ServiceManagement

class MainFlutterWindow: NSWindow {
  override func awakeFromNib() {
    let flutterViewController = FlutterViewController()
    self.contentViewController = flutterViewController
    // Fogata opens as a small splash window (lib/src/splash.dart), centred, which cannot
    // be resized and shows no window buttons; the application then gives it the main
    // window's size, smallest size and buttons (lib/src/window.dart).
    self.setContentSize(NSSize(width: 300, height: 360))
    self.minSize = NSSize(width: 300, height: 360)
    self.styleMask.remove(.resizable)
    self.center()

    // Fogata draws its own title bar: the content reaches the top edge, and an empty
    // compact toolbar makes the title bar as tall as the application's 40 pt header so
    // the traffic lights sit centred in it.
    self.title = "Fogata"
    self.titleVisibility = .hidden
    self.titlebarAppearsTransparent = true
    self.styleMask.insert(.fullSizeContentView)
    let toolbar = NSToolbar(identifier: "fogata")
    toolbar.showsBaselineSeparator = false
    self.toolbar = toolbar
    self.toolbarStyle = .unifiedCompact
    self.titlebarSeparatorStyle = .none
    for button in [NSWindow.ButtonType.closeButton, .miniaturizeButton, .zoomButton] {
      self.standardWindowButton(button)?.isHidden = true
    }

    // In full screen macOS draws the toolbar as an opaque bar over the header, so it is
    // shown only while windowed.
    let notifications = NotificationCenter.default
    notifications.addObserver(
      forName: NSWindow.willEnterFullScreenNotification, object: self, queue: .main
    ) { [weak self] _ in
      self?.toolbar?.isVisible = false
    }
    notifications.addObserver(
      forName: NSWindow.didExitFullScreenNotification, object: self, queue: .main
    ) { [weak self] _ in
      self?.toolbar?.isVisible = true
    }

    // Starts a new instance of the application, for it to start itself again (see
    // lib/src/restart.dart). Launch Services starts it, as the Finder would, so the system
    // sees the application itself start, with its own permissions.
    let restart = FlutterMethodChannel(
      name: "fogata/restart", binaryMessenger: flutterViewController.engine.binaryMessenger)
    restart.setMethodCallHandler { call, result in
      guard call.method == "startAgain" else {
        result(FlutterMethodNotImplemented)
        return
      }
      let options = call.arguments as? [String: Any]
      let configuration = NSWorkspace.OpenConfiguration()
      configuration.createsNewApplicationInstance = true
      configuration.activates = true
      configuration.arguments = options?["arguments"] as? [String] ?? []
      configuration.environment = options?["environment"] as? [String: String] ?? [:]
      NSWorkspace.shared.openApplication(
        at: Bundle.main.bundleURL, configuration: configuration
      ) { _, error in
        DispatchQueue.main.async {
          if let error = error {
            result(
              FlutterError(code: "open", message: error.localizedDescription, details: nil))
          } else {
            result(nil)
          }
        }
      }
    }

    // Opens Fogata when the person logs in (see lib/src/autostart.dart): the application
    // itself as a login item, which a sandboxed application may register. macOS 12 has no
    // such call for the application itself.
    let autostart = FlutterMethodChannel(
      name: "fogata/autostart", binaryMessenger: flutterViewController.engine.binaryMessenger)
    autostart.setMethodCallHandler { call, result in
      guard #available(macOS 13.0, *) else {
        result("unsupported")
        return
      }
      let service = SMAppService.mainApp
      switch call.method {
      case "get":
        result(loginItemState(service))
      case "set":
        let on = call.arguments as? Bool ?? false
        do {
          if on {
            try service.register()
          } else if service.status != .notRegistered && service.status != .notFound {
            try service.unregister()
          }
          result(loginItemState(service))
        } catch {
          result(FlutterError(code: "set", message: error.localizedDescription, details: nil))
        }
      default:
        result(FlutterMethodNotImplemented)
      }
    }

    RegisterGeneratedPlugins(registry: flutterViewController)

    super.awakeFromNib()
  }
}

/// How the login item stands, in the words lib/src/autostart.dart reads.
@available(macOS 13.0, *)
private func loginItemState(_ service: SMAppService) -> String {
  switch service.status {
  case .enabled: return "on"
  case .requiresApproval: return "approval"
  case .notRegistered, .notFound: return "off"
  @unknown default: return "off"
  }
}
