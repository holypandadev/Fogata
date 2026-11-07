import Cocoa
import FlutterMacOS

@main
class AppDelegate: FlutterAppDelegate {
  // Closing the window hides it while Fogata keeps running in the menu bar, or quits it
  // from the Dart side when that is turned off.
  override func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
    return false
  }

  // A click on the Dock icon brings back the window hidden in the menu bar.
  override func applicationShouldHandleReopen(
    _ sender: NSApplication, hasVisibleWindows flag: Bool
  ) -> Bool {
    if !flag {
      mainFlutterWindow?.makeKeyAndOrderFront(nil)
    }
    return true
  }

  override func applicationSupportsSecureRestorableState(_ app: NSApplication) -> Bool {
    return true
  }
}
