import 'dart:io';

import 'package:flutter/foundation.dart' show kReleaseMode;
import 'package:flutter/material.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart'
    show ExternalLibrary;
import 'package:flutter_webrtc/flutter_webrtc.dart'
    show WebRTC, onWebRtcEventError;

import 'src/app_state.dart';
import 'src/console.dart' show guardConsole, logConsoleIn;
import 'src/format.dart' show explain;
import 'src/icons.dart';
import 'src/l10n.dart';
import 'src/report.dart' show appErrorNames, noteAppError;
import 'src/restart.dart';
import 'src/rust/api/app.dart' as api;
import 'src/rust/frb_generated.dart';
import 'src/screens/move_account.dart';
import 'src/screens/report.dart' show reportNames;
import 'src/screens/shell.dart';
import 'src/screens/welcome.dart';
import 'src/splash.dart';
import 'src/theme.dart';
import 'src/widgets.dart';
import 'src/window.dart';

Future<void> main([List<String> args = const []]) async {
  // Before anything that may print: in a release build nothing of the start reaches the
  // console as it is (console.dart).
  guardConsole();
  launchArguments = args;
  WidgetsFlutterBinding.ensureInitialized();
  // macOS's own voice processing plays the others only while a microphone records: with
  // none, or with the microphone closed while muted, the call went silent. WebRTC's own
  // processing, as on Linux and Windows, plays whatever the microphone does. It has to be
  // chosen before WebRTC's first use.
  if (Platform.isMacOS) {
    await WebRTC.initialize(options: {'bypassVoiceProcessing': true});
  }
  await setUpWindow();
  await RustLib.init(externalLibrary: _rustLibrary());
  final String dataDir;
  try {
    dataDir = Platform.environment['FOGATA_DATA_DIR'] ?? api.defaultDataDir();
  } catch (e) {
    // Nowhere to keep its data: the window says why, and nothing starts.
    await _cannotStart(explain(e));
    return;
  }
  _noteErrors(dataDir);
  // The data folder is this user's alone before the window writes anything in it: another
  // user who could open the file that says Fogata runs could keep it from starting.
  try {
    api.prepareDataDir(dataDir: dataDir);
  } catch (e) {
    noteAppError(dataDir, 'the data folder cannot be made private: $e', null);
  }
  // A copy that started this one again is still quitting.
  await readRestart(dataDir);
  final wait = startedByRestart ? const Duration(seconds: 30) : Duration.zero;
  final bool only;
  try {
    only = await onlyCopy(dataDir, wait: wait);
  } on FileSystemException catch (e) {
    // The data folder cannot be written (not this user's, a disk that only reads, a full
    // one): the window says so. Left uncaught, this ended the start with the application
    // running and no window at all.
    await _cannotStart(
      '$e'.contains('No space left on device')
          ? l10n.errDiskFull
          : l10n.errDataFolder(dataDir),
    );
    return;
  }
  if (!only) exit(0);
  final state = AppState(dataDir);
  // The window's errors written to the log lose the names it knows, as the core's do.
  appErrorNames = () => reportNames(state);
  // The language and the theme, before the splash says anything.
  await state.prefs.load();
  await beforeQuitting(
    state.leaveBeforeQuit,
    keepRunning: () => state.prefs.tray,
  );
  state.callState.onRing = showIfHidden;
  runApp(FogataApp(state: state));
  final shownAt = DateTime.now();
  await state.boot();
  await finishSplash(
    splash: state.splash,
    updates: state.updates,
    started: state.started,
    askFriends:
        state.contacts.friends.isNotEmpty || state.contacts.groups.isNotEmpty,
    shownAt: shownAt,
    say: state.say,
    nameOf: (device) => state.contacts.friendPerson(device)?.name,
  );
}

/// Fogata cannot start, and never will as things are: the window says [why], and nothing
/// else starts.
Future<void> _cannotStart(String why) async {
  final state = AppState('')..startError = why;
  state.splash.showing = false;
  await becomeMainWindow();
  runApp(FogataApp(state: state));
}

/// Notes the window's errors in the day's log, for problem reports; in a release build the
/// console is told only their kind and where they happened (console.dart).
void _noteErrors(String dataDir) {
  logConsoleIn(dataDir);
  // What the video library's event channels fail with: noted here; the connection or
  // renderer that it came from fails by itself (the fork's FOGATA.md).
  onWebRtcEventError = (what, error, stack) => noteAppError(
    dataDir,
    'WebRTC event channel of $what failed: $error',
    stack,
  );
}

/// On macOS the Rust library is linked into the bridge plugin's framework, which is named
/// after the plugin. Elsewhere release builds open it by its own name from the
/// application's folder; other builds let the bridge also look for it in the source tree
/// (and where an environment variable says), for tests run from there.
ExternalLibrary? _rustLibrary() {
  if (Platform.isMacOS) {
    return ExternalLibrary.open('rust_lib_fogata.framework/rust_lib_fogata');
  }
  if (!kReleaseMode) return null;
  return ExternalLibrary.open(
    Platform.isWindows ? 'fogata_ffi.dll' : 'libfogata_ffi.so',
  );
}

/// The application: built again only when the theme changes. What shows the state
/// depends on [AppScope], which rebuilds it.
class FogataApp extends StatefulWidget {
  const FogataApp({super.key, required this.state});
  final AppState state;

  @override
  State<FogataApp> createState() => _FogataAppState();
}

class _FogataAppState extends State<FogataApp> {
  late Look _look = widget.state.prefs.look;
  late String _language = widget.state.prefs.language;

  @override
  void initState() {
    super.initState();
    widget.state.prefs.addListener(_changed);
  }

  @override
  void dispose() {
    widget.state.prefs.removeListener(_changed);
    super.dispose();
  }

  void _changed() {
    final look = widget.state.prefs.look;
    if (look != _look) setState(() => _look = look);
    final language = widget.state.prefs.language;
    if (language != _language) {
      setState(() => _language = language);
      // Words said without a context come from the language in use as they build:
      // everything builds again, keeping its state.
      void rebuild(Element e) {
        e.markNeedsBuild();
        e.visitChildren(rebuild);
      }

      (context as Element).visitChildren(rebuild);
    }
  }

  @override
  Widget build(BuildContext context) => AppScope(
    state: widget.state,
    child: MaterialApp(
      title: 'Fogata',
      debugShowCheckedModeBanner: false,
      locale: localeOf(_language),
      supportedLocales: [for (final code in languages.keys) localeOf(code)],
      localizationsDelegates: L10n.localizationsDelegates,
      theme: Fg.lightTokens.themeData,
      darkTheme: _look.tokens.themeData,
      themeMode: _look.mode,
      home: const _Root(),
      builder: appFrame,
    ),
  );
}

class _Root extends StatelessWidget {
  const _Root();

  @override
  Widget build(BuildContext context) {
    final splash = AppScope.read(context).splash;
    return ListenableBuilder(
      listenable: splash,
      builder: (context, root) => switch ((splash.showing, splash.leaving)) {
        (false, _) => root!,
        // The window turns into the main one meanwhile.
        (true, true) => ColoredBox(color: context.fg.bg),
        (true, false) => Material(
          type: MaterialType.transparency,
          child: SplashCard(
            step: splash.step,
            done: splash.done,
            total: splash.total,
            from: splash.from,
          ),
        ),
      },
      child: const _App(),
    );
  }
}

/// What the main window shows.
class _App extends StatelessWidget {
  const _App();

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = context.fg;
    final Widget body;
    if (state.startError != null && !state.started) {
      body = _Starting(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            // A long reason (one that names a folder) reads as a paragraph, not as one
            // line from edge to edge of a wide window.
            ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 560),
              child: Text(
                state.startError!,
                textAlign: TextAlign.center,
                style: tx(c.red),
              ),
            ),
            if (state.dataDir.isNotEmpty) ...[
              const SizedBox(height: 16),
              FgButton(
                text: l10n.commonTryAgain,
                icon: Ic.retry,
                onTap: state.retryStart,
              ),
            ],
          ],
        ),
      );
    } else if (state.moveState?.kind == 'moved' ||
        state.moveState?.kind == 'moving') {
      body = const MovedScreen();
    } else if (state.bringing || state.restoring || !state.hasProfile) {
      body = const Welcome();
    } else if (!state.started) {
      body = const _Starting(child: Spinner(size: 18));
    } else {
      body = const Shell();
    }
    return Material(
      color: c.bg,
      child: DefaultTextStyle(
        style: tx(c.fg),
        child: WithMoveQuestion(child: body),
      ),
    );
  }
}

/// Fogata starting, or what kept it from starting, under the title bar, so the window
/// can still be moved and closed.
class _Starting extends StatelessWidget {
  const _Starting({required this.child});
  final Widget child;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        TitleBar(
          children: [
            IgnorePointer(
              child: Image.asset(
                'assets/images/logo.png',
                width: 18,
                height: 18,
              ),
            ),
            const SizedBox(width: 8),
            IgnorePointer(
              child: Text(
                'Fogata',
                style: tx(c.fg, size: 14, weight: w600, spacing: -.15),
              ),
            ),
            const Spacer(),
          ],
        ),
        Expanded(
          child: Center(
            child: Padding(padding: const EdgeInsets.all(32), child: child),
          ),
        ),
      ],
    );
  }
}
