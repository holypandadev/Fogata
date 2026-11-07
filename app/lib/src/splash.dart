import 'dart:async';

import 'package:flutter/material.dart';
import 'package:window_manager/window_manager.dart';

import 'host.dart';
import 'l10n.dart';
import 'rust/api/update.dart' as api;
import 'state/piece.dart';
import 'theme.dart';
import 'update.dart';
import 'window.dart';

/// What the splash says Fogata is doing.
enum SplashStep {
  starting,
  connecting,
  checking,
  downloading,
  installing,
  restarting;

  String get text => switch (this) {
    starting => l10n.splashStarting,
    connecting => l10n.splashConnecting,
    checking => l10n.splashChecking,
    downloading => l10n.splashDownloading,
    installing => l10n.splashInstalling,
    restarting => l10n.splashRestarting,
  };
}

/// The small window Fogata opens first at every start: the logo and what
/// is happening, until the main window shows. The window itself is made small in the
/// runners and in [setUpWindow]; [finishSplash] makes it the main one.
class Splash extends Piece {
  /// The splash shows instead of the application.
  bool showing = true;

  /// The window is turning into the main one: nothing shows for that moment.
  bool leaving = false;

  SplashStep step = SplashStep.starting;

  /// While an update downloads: bytes here of [total], and from whose computer.
  int done = 0;
  int total = 0;
  String? from;

  void show(SplashStep step) {
    if (!showing || this.step == step) return;
    this.step = step;
    changed();
  }
}

/// At least this long, so the splash is seen rather than flashing.
const _shortest = Duration(milliseconds: 600);

/// The longest the splash waits for friends and the releases page to say whether there is
/// a newer version.
const _askFor = Duration(seconds: 6);

/// With nobody to ask, the core says it is checking (the releases page) within this long,
/// or it does not check at all.
const _checkStarts = Duration(milliseconds: 1500);

/// A download that would take longer than this is left to go on behind the main window,
/// unless this version must update.
const _longDownload = Duration(minutes: 3);

/// A download that stops moving for this long is left to go on behind the main window.
const _stalled = Duration(seconds: 15);

/// After Fogata started (or found nothing to start): asks friends and the releases page
/// for a newer version a few seconds at most, and installs one that is here or comes,
/// which restarts Fogata; otherwise, once the splash showed long enough, it becomes the
/// main window. Returns false when Fogata is restarting into a new version.
Future<bool> finishSplash({
  required Splash splash,
  required Updates updates,
  required bool started,
  required bool askFriends,
  required DateTime shownAt,
  required void Function(String, {bool error}) say,
  required String? Function(String device) nameOf,
}) async {
  if (started) {
    final failed = await _updateAtStart(
      splash,
      updates,
      askFriends: askFriends,
      nameOf: nameOf,
    );
    if (failed == null) return false;
    if (failed.isNotEmpty) say(l10n.updateFailed(failed), error: true);
  }
  final shown = DateTime.now().difference(shownAt);
  if (shown < _shortest) await Future<void>.delayed(_shortest - shown);
  splash
    ..leaving = true
    ..changed();
  await _frame();
  await becomeMainWindow();
  await _frame();
  splash
    ..showing = false
    ..leaving = false
    ..changed();
  return true;
}

/// The next frame drawn, or a moment where none is drawn on its own (tests draw frames
/// only when they pump).
Future<void> _frame() => Future.any([
  WidgetsBinding.instance.endOfFrame,
  Future<void>.delayed(const Duration(milliseconds: 100)),
]);

/// The update steps of the splash. Null when an update was installed and Fogata quits;
/// otherwise why an install failed, or empty when none was tried.
Future<String?> _updateAtStart(
  Splash splash,
  Updates updates, {
  required bool askFriends,
  required String? Function(String device) nameOf,
}) async {
  splash.show(SplashStep.checking);
  try {
    await api.checkForUpdate();
  } catch (_) {}
  await updates.refresh();
  bool on(Set<api.UiUpdateState> states) =>
      states.contains(updates.status?.state);
  const coming = {
    api.UiUpdateState.downloading,
    api.UiUpdateState.verifying,
    api.UiUpdateState.ready,
  };
  // Friends answer as they connect, and the releases page in a moment: until a newer
  // version is found, or those asked said there is none, a few seconds at most.
  if (!on(coming)) {
    final start = DateTime.now();
    final until = start.add(_askFor);
    var asked = on({api.UiUpdateState.checking});
    while (DateTime.now().isBefore(until)) {
      await Future<void>.delayed(const Duration(milliseconds: 200));
      await updates.refresh();
      if (on({api.UiUpdateState.checking})) asked = true;
      if (on(coming) || (asked && on({api.UiUpdateState.upToDate}))) break;
      if (!asked &&
          !askFriends &&
          DateTime.now().difference(start) > _checkStarts) {
        break;
      }
    }
  }
  // The download is waited for while it moves, as long as it takes when this version
  // must update.
  if (on({api.UiUpdateState.downloading, api.UiUpdateState.verifying})) {
    splash.show(SplashStep.downloading);
    final start = DateTime.now();
    var progressAt = start;
    var last = -1;
    while (on({api.UiUpdateState.downloading, api.UiUpdateState.verifying})) {
      final status = updates.status!;
      final done = status.done.toInt(), total = status.total.toInt();
      splash
        ..done = done
        ..total = total
        ..from = switch (status.fromDevice) {
          final device? => nameOf(device),
          null => null,
        }
        ..changed();
      final now = DateTime.now();
      if (done != last) {
        last = done;
        progressAt = now;
      }
      final spent = now.difference(start);
      // Its pace so far says when it would end; one that stalls is not waited for.
      final ends = done > 0 && total > 0 && spent > const Duration(seconds: 5)
          ? spent * (total / done)
          : Duration.zero;
      if (now.difference(progressAt) > _stalled ||
          (!status.required_ && ends > _longDownload)) {
        return '';
      }
      await Future<void>.delayed(const Duration(milliseconds: 250));
      await updates.refresh();
    }
  }
  if (!updates.ready) return '';
  splash.show(SplashStep.installing);
  final failed = await updates.install(quit: false);
  if (failed != null) return failed;
  splash.show(SplashStep.restarting);
  await Future<void>.delayed(const Duration(milliseconds: 500));
  unawaited(quitApplication());
  return null;
}

/// The splash itself: the animated logo and the step under it, in a dark card with
/// round corners (on Windows and macOS the system rounds the window's own).
class SplashCard extends StatelessWidget {
  const SplashCard({
    super.key,
    required this.step,
    this.done = 0,
    this.total = 0,
    this.from,
  });

  final SplashStep step;
  final int done;
  final int total;

  /// The name of whoever the update comes from.
  final String? from;

  static const _card = Color(0xFF0A0A0A);

  @override
  Widget build(BuildContext context) {
    final round = onLinux;
    final status = tx(
      const Color(0xFF9A9A9A),
      size: 13,
      height: 1.7,
      spacing: 13 * .12,
      monospace: true,
    ).copyWith(fontStyle: FontStyle.italic);
    final small = tx(
      const Color(0xFF6C6C6C),
      size: 11,
      height: 1.7,
      spacing: 11 * .06,
      monospace: true,
    ).copyWith(fontStyle: FontStyle.italic);
    String mb(int bytes) => (bytes / 1e6).round().toString();
    return GestureDetector(
      behavior: HitTestBehavior.opaque,
      onPanStart: ownTitleBar ? (_) => windowManager.startDragging() : null,
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: _card,
          borderRadius: round ? BorderRadius.circular(22) : null,
          border: round ? Border.all(color: const Color(0xFF1D1D1D)) : null,
        ),
        // A window of its own size: the system's text size does not change it.
        child: MediaQuery.withNoTextScaling(
          child: Stack(
            children: [
              Positioned(
                top: 40,
                left: 0,
                right: 0,
                child: Center(
                  child: Image.asset(
                    'assets/splash/splash.webp',
                    width: 220,
                    height: 220,
                    filterQuality: FilterQuality.medium,
                    gaplessPlayback: true,
                  ),
                ),
              ),
              // Over the bottom of the picture, which is clear, when it takes more lines.
              Positioned(
                left: 16,
                right: 16,
                bottom: 46,
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Text(
                      step.text.toUpperCase(),
                      key: const ValueKey('splash-step'),
                      textAlign: TextAlign.center,
                      style: status,
                    ),
                    if (step == SplashStep.downloading) ...[
                      const SizedBox(height: 10),
                      Container(
                        width: 150,
                        height: 4,
                        alignment: Alignment.centerLeft,
                        decoration: BoxDecoration(
                          color: const Color(0xFF1F1F1F),
                          borderRadius: BorderRadius.circular(999),
                        ),
                        child: FractionallySizedBox(
                          widthFactor: total > 0
                              ? (done / total).clamp(0, 1)
                              : 0,
                          child: Container(
                            decoration: BoxDecoration(
                              borderRadius: BorderRadius.circular(999),
                              gradient: const LinearGradient(
                                colors: [Color(0xFFFF6A1A), Color(0xFFFFB347)],
                              ),
                            ),
                          ),
                        ),
                      ),
                      if (total > 0)
                        Text(
                          from == null
                              ? l10n.splashProgress(mb(done), mb(total))
                              : l10n.splashProgressFrom(
                                  mb(done),
                                  mb(total),
                                  from!,
                                ),
                          textAlign: TextAlign.center,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: small,
                        ),
                    ],
                  ],
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
