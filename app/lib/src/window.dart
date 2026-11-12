import 'dart:async';
import 'dart:io';
import 'dart:math' as math;
import 'dart:ui'
    as ui
    show ImageByteFormat, PlatformDispatcher, instantiateImageCodec;
import 'dart:ui' show AppExitResponse;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:screen_retriever/screen_retriever.dart';
import 'package:tray_manager/tray_manager.dart';
import 'package:window_manager/window_manager.dart';

import 'host.dart';
import 'icons.dart';
import 'l10n.dart';
import 'moving.dart';
import 'report.dart' show flushAppErrors, noteAppError;
import 'rust/api/tray.dart' as linux_tray;
import 'shown.dart';
import 'theme.dart';
import 'widgets.dart';

/// The application draws its own title bar inside its header: on Windows and Linux with
/// its own window buttons on the right, on macOS around the system's traffic lights.
bool get ownTitleBar =>
    Platform.isLinux || Platform.isWindows || Platform.isMacOS;

/// Prepares the window before the first frame. Linux and macOS hide the system title bar
/// in their runners; Windows does it here. The window opens as the splash
/// (splash.dart): Linux and macOS make it so in their runners, and on Windows the window
/// made for the main one shrinks around its middle here, before anything shows.
Future<void> setUpWindow() async {
  if (!ownTitleBar) return;
  await windowManager.ensureInitialized();
  if (Platform.isWindows) {
    await windowManager.setTitleBarStyle(TitleBarStyle.hidden);
    _mainBounds = await windowManager.getBounds();
    await windowManager.setResizable(false);
    await windowManager.setBounds(
      Rect.fromCenter(
        center: _mainBounds!.center,
        width: splashSize.width,
        height: splashSize.height,
      ),
    );
  }
  if (Platform.isMacOS) windowManager.addListener(_FullScreenWatcher());
  windowManager.addListener(_ShownWatcher());
}

/// The splash window's size.
const splashSize = Size(300, 360);

/// The radius of the main window's corners that the application rounds itself: on Linux
/// where the desktop composites windows and the window is not maximized, tiled or full
/// screen (the runner says so: desktop_channel.cc); 0 elsewhere, and on Windows and macOS,
/// whose systems round their windows' own.
final windowCorner = ValueNotifier<double>(0);

/// Follows what the Linux runner says of the corners.
void _followCorners() {
  const MethodChannel('fogata/desktop').setMethodCallHandler((call) async {
    if (call.method == 'windowCorner' && call.arguments is num) {
      windowCorner.value = (call.arguments as num).toDouble();
    }
  });
}

/// Where Windows put the main window, while the splash shows in its middle.
Rect? _mainBounds;

/// The main window's smallest size, and the size it opens at, on a screen of [screen]
/// whose work area (the screen without the taskbar, the Dock or the menu bar) is [work]:
/// 960 by 640 at least, 1440 by 900 (or the screen less a margin, if smaller) when it
/// opens, and neither ever larger than the work area: a window that may not be made
/// smaller than the screen it is on has its bottom (the message box, the call's buttons)
/// under the taskbar, and cannot be brought back (a 1080p laptop at 175 %, 1280 by 800
/// at 125 %).
({Size minimum, Size opening}) mainWindowSizes({
  required Size screen,
  required Size work,
}) {
  final minimum = Size(math.min(960, work.width), math.min(640, work.height));
  return (
    minimum: minimum,
    opening: Size(
      math.min(
        work.width,
        math.max(minimum.width, math.min(1440, screen.width - 80)),
      ),
      math.min(
        work.height,
        math.max(minimum.height, math.min(900, screen.height - 80)),
      ),
    ),
  );
}

/// The screen the window is on, and its work area, in logical pixels: the display whose
/// work area holds [point], else the main one. Without an answer from the system, the
/// whole first display stands for both.
Future<({Size screen, Size work})> _screenAt(Offset point) async {
  try {
    final displays = await screenRetriever.getAllDisplays();
    final primary = await screenRetriever.getPrimaryDisplay();
    final display = displays.firstWhere((d) {
      final at = d.visiblePosition;
      final size = d.visibleSize;
      return at != null && size != null && (at & size).contains(point);
    }, orElse: () => primary);
    return (screen: display.size, work: display.visibleSize ?? display.size);
  } catch (_) {
    final display = ui.PlatformDispatcher.instance.displays.first;
    final screen = display.size / display.devicePixelRatio;
    return (screen: screen, work: screen);
  }
}

/// The splash window becomes the main one: its frame, window buttons, smallest size, and
/// 1440 by 900 (or the screen, if smaller) in the middle of the screen; on Wayland, where
/// the compositor places windows, wherever it puts it. See [mainWindowSizes].
Future<void> becomeMainWindow() async {
  if (!ownTitleBar) return;
  try {
    if (Platform.isLinux) {
      _followCorners();
      final corner = await const MethodChannel('fogata/desktop')
          .invokeMethod<Object?>('mainWindow');
      if (corner is num) windowCorner.value = corner.toDouble();
      return;
    }
    await windowManager.setResizable(true);
    final now = await windowManager.getBounds();
    final (:screen, :work) = await _screenAt((_mainBounds ?? now).center);
    final sizes = mainWindowSizes(screen: screen, work: work);
    await windowManager.setMinimumSize(sizes.minimum);
    if (Platform.isMacOS) {
      await windowManager.setTitleBarStyle(
        TitleBarStyle.hidden,
        windowButtonVisibility: true,
      );
    }
    // Windows' runner already made it fit the work area (main.cpp).
    final bounds =
        _mainBounds ??
        Rect.fromCenter(
          center: now.center,
          width: sizes.opening.width,
          height: sizes.opening.height,
        );
    await windowManager.setBounds(bounds);
  } catch (_) {}
}

/// Keeps [windowShown] up to date as the window is minimized and brought back; a window
/// that gets the focus is surely seen. [windowFocused] follows the focus.
class _ShownWatcher with WindowListener {
  @override
  void onWindowMinimize() => windowShown.value = false;

  @override
  void onWindowRestore() => windowShown.value = true;

  @override
  void onWindowFocus() {
    windowShown.value = true;
    windowFocused.value = true;
  }

  @override
  void onWindowBlur() => windowFocused.value = false;
}

/// Runs [before] when the application quits (the tray's menu, its own menu, Cmd+Q, the
/// system), then lets it go. The room is left this way, so the others see this device
/// leave at once rather than when it stops answering. Closing the window only hides it
/// while [keepRunning] says so: the application goes on in the tray, which brings the
/// window back; otherwise closing it quits.
Future<void> beforeQuitting(
  Future<void> Function() before, {
  required bool Function() keepRunning,
}) async {
  Future<void>? leaving;
  Future<void> leave() => leaving ??= () async {
    try {
      await before().timeout(const Duration(seconds: 3));
    } catch (_) {}
    // The window's errors of the last second wait to be noted: on the disk before it goes.
    try {
      await flushAppErrors().timeout(const Duration(seconds: 1));
    } catch (_) {}
  }();
  // Whether closing the window hides it now: not once quitting has begun.
  bool hides() => leaving == null && _inTray && keepRunning();

  // The listener stays registered with the binding for the life of the application.
  AppLifecycleListener(
    // Hidden or minimized, as the system tells it.
    onStateChange: (state) => switch (state) {
      AppLifecycleState.hidden ||
      AppLifecycleState.paused => windowShown.value = false,
      AppLifecycleState.resumed ||
      AppLifecycleState.inactive => windowShown.value = true,
      AppLifecycleState.detached => null,
    },
    onExitRequested: () async {
      // Windows can turn closing the window into a request to quit, which comes here
      // before the window hears of it.
      if (Platform.isWindows && hides()) {
        await _hideWindow();
        return AppExitResponse.cancel;
      }
      await leave();
      return AppExitResponse.exit;
    },
  );
  if (!ownTitleBar) {
    quitApplication = () async {
      await leave();
      exit(0);
    };
    return;
  }
  var destroying = false;
  Future<void> quit() async {
    await leave();
    // On Linux destroying the window closes it once more, which comes back here.
    if (destroying) return;
    destroying = true;
    try {
      if (Platform.isLinux) {
        await linux_tray.hideTray();
      } else {
        await trayManager.destroy();
      }
    } catch (_) {}
    await windowManager.destroy();
  }

  quitApplication = quit;
  await windowManager.setPreventClose(true);
  windowManager.addListener(
    _CloseWatcher(() async {
      if (hides()) {
        await _hideWindow();
      } else {
        await quit();
      }
    }),
  );
  await _setUpTray(quit);
}

/// Quits the application as the tray's menu does: leaving the room first.
Future<void> Function() quitApplication = () async => exit(0);

class _CloseWatcher with WindowListener {
  _CloseWatcher(this.close);
  final Future<void> Function() close;

  @override
  void onWindowClose() => close();
}

/// Whether the tray icon is there to bring the window back; the setting to keep running
/// shows only then.
bool get hasTray => _inTray;
var _inTray = false;

/// The desktop shows no tray icons (Linux without a StatusNotifier host, as GNOME without
/// its AppIndicator extension): an icon made there is never seen, so none is made, and
/// closing the window quits.
bool get trayNotShown => _trayNotShown;
var _trayNotShown = false;

/// The icon in the system tray (Windows' notification area, a StatusNotifier host on
/// Linux, macOS's menu bar), with a menu to open the window or quit. A click on the icon
/// opens the window.
Future<void> _setUpTray(Future<void> Function() quit) async {
  if (Platform.isLinux) {
    // followLinuxTray says when there is an icon to bring the window back.
    _trayNotShown = !await _setUpLinuxTray(quit);
    return;
  }
  try {
    await trayManager.setIcon(
      Platform.isWindows
          ? 'assets/tray/tray.ico'
          : 'assets/tray/tray-template.png',
      // Drawn by macOS in the menu bar's colour, like the system's own icons.
      isTemplate: Platform.isMacOS,
    );
    await trayManager.setToolTip('Fogata');
    await _trayMenu();
    trayManager.addListener(_TrayWatcher(quit));
    _inTray = true;
  } catch (_) {}
}

Future<void> _trayMenu() => trayManager.setContextMenu(
  Menu(
    items: [
      MenuItem(key: 'open', label: l10n.windowTrayOpen),
      MenuItem.separator(),
      MenuItem(key: 'quit', label: l10n.windowTrayQuit),
    ],
  ),
);

/// Says the tray icon's menu again in the language in use.
Future<void> relabelTray() async {
  if (!_inTray) return;
  try {
    if (Platform.isLinux) {
      await _showLinuxTray();
    } else {
      await _trayMenu();
    }
  } catch (_) {}
}

/// Linux's icon picture, read once: its size and its pixels in rows of red, green, blue
/// and alpha bytes.
(int, int, Uint8List)? _linuxIcon;

/// Shows Linux's icon with its menu in the language in use, in place of one shown before;
/// false when the desktop shows none.
Future<bool> _showLinuxTray() async {
  final icon = _linuxIcon ??= await () async {
    final png = await rootBundle.load('assets/tray/tray.png');
    final codec = await ui.instantiateImageCodec(png.buffer.asUint8List());
    final image = (await codec.getNextFrame()).image;
    final pixels = await image.toByteData(
      format: ui.ImageByteFormat.rawStraightRgba,
    );
    final width = image.width, height = image.height;
    image.dispose();
    if (pixels == null) return null;
    return (
      width,
      height,
      pixels.buffer.asUint8List(pixels.offsetInBytes, pixels.lengthInBytes),
    );
  }();
  if (icon == null) return false;
  return linux_tray.showTray(
    title: 'Fogata',
    width: icon.$1,
    height: icon.$2,
    rgba: icon.$3,
    items: [
      linux_tray.UiTrayItem(key: 'open', label: l10n.windowTrayOpen),
      const linux_tray.UiTrayItem(key: '', label: ''),
      linux_tray.UiTrayItem(key: 'quit', label: l10n.windowTrayQuit),
    ],
  );
}

/// Linux's icon, which the Rust library serves on the session bus as a StatusNotifierItem
/// with its menu; false when the desktop shows none. The menu opens from the other mouse
/// button.
Future<bool> _setUpLinuxTray(Future<void> Function() quit) async {
  try {
    final shown = await _showLinuxTray();
    // Listened to only once there is an icon: the bridge's stream is never let go of (a
    // cancel would wait for an event that never comes).
    if (shown) followLinuxTray(linux_tray.trayEvents(), quit);
    return shown;
  } catch (_) {
    return false;
  }
}

/// Follows the clicks on Linux's icon and its menu. Should they end, or fail, the icon
/// brings nothing back any more: closing the window quits from then on, a window hidden in
/// the tray comes back, and a failure is noted as the window's other errors are.
@visibleForTesting
void followLinuxTray(
  Stream<linux_tray.UiTrayEvent> events,
  Future<void> Function() quit,
) {
  _inTray = true;
  void ended() {
    if (!_inTray) return;
    _inTray = false;
    unawaited(() async {
      try {
        if (!await windowManager.isVisible()) await showWindow();
      } catch (_) {}
    }());
  }

  events.listen(
    (event) {
      switch (event.kind) {
        case linux_tray.UiTrayEventKind.activate:
          showWindow();
        case linux_tray.UiTrayEventKind.item:
          _chosen(event.key, quit);
      }
    },
    onError: (Object e, StackTrace stack) {
      ui.PlatformDispatcher.instance.onError?.call(
        'the tray icon\'s events failed: $e',
        stack,
      );
      ended();
    },
    onDone: ended,
    cancelOnError: true,
  );
}

/// The tray menu's entry [key] was chosen.
void _chosen(String key, Future<void> Function() quit) {
  switch (key) {
    case 'open':
      showWindow();
    case 'quit':
      quit();
  }
}

class _TrayWatcher with TrayListener {
  _TrayWatcher(this.quit);
  final Future<void> Function() quit;

  @override
  void onTrayIconMouseDown() => showWindow();

  @override
  void onTrayIconRightMouseDown() {
    // Windows closes the menu on a click elsewhere only when the application is in front.
    // ignore: deprecated_member_use
    trayManager.popUpContextMenu(bringAppToFront: Platform.isWindows);
  }

  @override
  void onTrayMenuItemClick(MenuItem menuItem) =>
      _chosen(menuItem.key ?? '', quit);
}

/// Makes this the only copy of the application on [dataDir], where two copies of one
/// device would each write its state. Returns false when another copy runs there: that
/// one is asked to show its window, and this one should quit before showing its own.
/// With [wait], another copy is given that long to quit first (it is starting this one
/// again).
Future<bool> onlyCopy(String dataDir, {Duration wait = Duration.zero}) async {
  final folder = Directory('$dataDir/window');
  await folder.create(recursive: true);
  final running = await File('${folder.path}/running')
      .open(mode: FileMode.append);
  final until = DateTime.now().add(wait);
  while (true) {
    try {
      // The system lets go of the lock when this copy ends, however it ends.
      await running.lock(FileLock.exclusive);
      break;
    } on FileSystemException {
      if (DateTime.now().isBefore(until)) {
        await Future<void>.delayed(const Duration(milliseconds: 200));
        continue;
      }
    }
    await running.close();
    await File('${folder.path}/show')
        .writeAsString('${DateTime.now().millisecondsSinceEpoch}');
    return false;
  }
  _running = running;
  watchForShow(
    folder,
    showWindow,
    (error) =>
        noteAppError(dataDir, 'watching for another copy failed: $error', null),
  );
  return true;
}

/// Calls [show] whenever another copy writes the `show` file in [folder]. When the watch
/// fails (the system's limit of watchers, or the folder gone) the error goes to [failed]
/// and the watch is made again after [retry], which doubles up to [longest] while it
/// keeps failing: a copy that stopped listening would not show its window when another
/// is started. A request written while the watch was down is answered when it is made
/// again. The result ends the watch. [watch] stands in for the system's watch in tests,
/// and [known] tells them when the request already there (none, or one answered before) is
/// known: one written after that is new.
@visibleForTesting
void Function() watchForShow(
  Directory folder,
  void Function() show,
  void Function(Object error) failed, {
  Duration retry = const Duration(seconds: 2),
  Duration longest = const Duration(minutes: 1),
  Stream<FileSystemEvent> Function(Directory folder)? watch,
  void Function()? known,
}) {
  StreamSubscription<FileSystemEvent>? watching;
  Timer? waiting;
  var ended = false;
  var delay = retry;
  var armedAt = DateTime.now();
  final request = File('${folder.path}/show');

  // The request as last answered: its time as the file system keeps it (to the second on
  // some) and what it says, which its writer makes new each time. Compared with that, and
  // not with the clock, a request is told whatever the resolution of the file's time.
  String? answered;
  Future<String?> signature() async {
    try {
      final time = (await request.lastModified()).microsecondsSinceEpoch;
      return '$time|${await request.readAsString()}';
    } on Exception {
      // No request, or no reading of it.
      return null;
    }
  }

  Future<void> settled = signature().then((now) {
    answered = now;
    known?.call();
  });

  Future<void> answerMissed() async {
    await settled;
    final now = await signature();
    if (ended || now == null || now == answered) return;
    answered = now;
    show();
  }

  void arm() {
    if (ended) return;
    armedAt = DateTime.now();

    void lost(Object error) {
      watching?.cancel();
      watching = null;
      if (ended || waiting != null) return;
      failed(error);
      // A watch that held for a while starts the delays again.
      if (DateTime.now().difference(armedAt) > longest) delay = retry;
      waiting = Timer(delay, () {
        waiting = null;
        delay = delay * 2 > longest ? longest : delay * 2;
        arm();
        unawaited(answerMissed());
      });
    }

    try {
      final events = watch != null
          ? watch(folder)
          : folder.watch(
              events: FileSystemEvent.create | FileSystemEvent.modify,
            );
      watching = events
          .where((event) => event.path.endsWith('show'))
          .listen(
            (_) {
              show();
              settled = settled.then((_) => signature()).then((now) {
                answered = now ?? answered;
              });
            },
            onError: lost,
            onDone: () => lost('the watch ended'),
            cancelOnError: true,
          );
    } on Object catch (error) {
      lost(error);
    }
  }

  arm();
  return () {
    ended = true;
    waiting?.cancel();
    watching?.cancel();
  };
}

/// The locked file that says this copy runs, kept open while it does: a file nothing
/// holds gets closed, and the lock goes with it. Nothing reads it, so the release build
/// would drop it without the pragma.
@pragma('vm:entry-point')
// ignore: unused_element
RandomAccessFile? _running;

/// Brings the window back from the tray, or from behind others, and gives it the focus.
Future<void> showWindow() async {
  if (await windowManager.isMinimized()) await windowManager.restore();
  await windowManager.show();
  await windowManager.focus();
  windowShown.value = true;
}

/// Shows the window if it is hidden in the tray, so a call ringing there is seen.
Future<void> showIfHidden() async {
  if (!_inTray || await windowManager.isVisible()) return;
  await showWindow();
}

/// Hides the window, the application going on in the tray. On macOS it leaves full screen
/// first, or its empty space would stay behind.
Future<void> _hideWindow() async {
  if (_fullScreen.value) {
    final left = Completer<void>();
    void check() {
      if (!_fullScreen.value && !left.isCompleted) left.complete();
    }

    _fullScreen.addListener(check);
    await windowManager.setFullScreen(false);
    await left.future.timeout(const Duration(seconds: 2), onTimeout: () {});
    _fullScreen.removeListener(check);
  }
  await windowManager.hide();
  windowShown.value = false;
}

/// Whether the macOS window is in full screen, where the traffic lights hide.
final _fullScreen = ValueNotifier(false);

class _FullScreenWatcher with WindowListener {
  @override
  void onWindowEnterFullScreen() => _fullScreen.value = true;

  @override
  void onWindowLeaveFullScreen() => _fullScreen.value = false;
}

/// Fills the screen with the window, or brings it back.
Future<void> setWindowFullScreen(bool on) async {
  if (await windowManager.isFullScreen() != on) {
    await windowManager.setFullScreen(on);
  }
}

Future<void> _toggleMaximized() async {
  if (await windowManager.isMaximized()) {
    await windowManager.unmaximize();
  } else {
    await windowManager.maximize();
  }
}

/// A header that works as the window's title bar: dragging its empty parts moves the
/// window and a double click maximizes it. The window buttons go at the end of [children];
/// on macOS the start leaves room for the traffic lights.
class TitleBar extends StatelessWidget {
  const TitleBar({
    super.key,
    required this.children,
    this.height = 40,
    this.padding = const EdgeInsets.only(left: 14, right: 8),
  });

  final List<Widget> children;
  final double height;
  final EdgeInsets padding;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return Container(
      height: height,
      decoration: BoxDecoration(
        // The grey of the side columns, so the bar reads as the window's frame.
        color: c.bg1,
        border: Border(bottom: BorderSide(color: c.border)),
      ),
      child: Stack(
        fit: StackFit.expand,
        children: [
          if (ownTitleBar)
            Positioned.fill(
              child: GestureDetector(
                behavior: HitTestBehavior.opaque,
                onPanStart: (_) => windowManager.startDragging(),
                onDoubleTap: _toggleMaximized,
              ),
            ),
          ValueListenableBuilder(
            valueListenable: _fullScreen,
            builder: (context, fullScreen, _) => Padding(
              // Room for the traffic lights, except in full screen.
              padding: padding.copyWith(
                left: padding.left + (onMacOS && !fullScreen ? 70 : 0),
              ),
              child: Row(
                children: [
                  ...children,
                  if (ownTitleBar && !onMacOS) const WindowButtons(),
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }
}

/// Minimize, maximize or restore, and close, for Windows and Linux.
class WindowButtons extends StatefulWidget {
  const WindowButtons({super.key});

  @override
  State<WindowButtons> createState() => _WindowButtonsState();
}

class _WindowButtonsState extends State<WindowButtons> with WindowListener {
  bool _maximized = false;

  @override
  void initState() {
    super.initState();
    windowManager.addListener(this);
    windowManager.isMaximized().then((m) {
      if (mounted) setState(() => _maximized = m);
    });
  }

  @override
  void dispose() {
    windowManager.removeListener(this);
    super.dispose();
  }

  @override
  void onWindowMaximize() => setState(() => _maximized = true);

  @override
  void onWindowUnmaximize() => setState(() => _maximized = false);

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Container(
          width: 1,
          height: 18,
          color: c.border,
          margin: const EdgeInsets.only(left: 10, right: 6),
        ),
        _Button(
          icon: Ic.minimize,
          tip: l10n.windowMinimize,
          onTap: windowManager.minimize,
        ),
        const SizedBox(width: 4),
        _Button(
          icon: _maximized ? Ic.restore : Ic.maximize,
          tip: _maximized ? l10n.windowRestore : l10n.windowMaximize,
          onTap: _toggleMaximized,
        ),
        const SizedBox(width: 4),
        _Button(
          icon: Ic.close,
          tip: l10n.commonClose,
          onTap: windowManager.close,
          danger: true,
        ),
      ],
    );
  }
}

class _Button extends StatelessWidget {
  const _Button({
    required this.icon,
    required this.tip,
    required this.onTap,
    this.danger = false,
  });

  final Ic icon;
  final String tip;
  final VoidCallback onTap;
  final bool danger;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return Pressable(
      onTap: onTap,
      tooltip: tip,
      builder: (context, hovered) => AnimatedContainer(
        duration: const Duration(milliseconds: 140),
        width: 30,
        height: 28,
        alignment: Alignment.center,
        decoration: BoxDecoration(
          color: hovered ? (danger ? c.redSolid : c.hover) : null,
          borderRadius: BorderRadius.circular(999),
        ),
        child: FgIcon(
          icon,
          size: 14,
          color: hovered ? (danger ? Colors.white : c.fg) : c.fg2,
        ),
      ),
    );
  }
}
