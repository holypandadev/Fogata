import 'dart:async';

import 'package:flutter/material.dart';

import 'format.dart' show explain;
import 'icons.dart';
import 'l10n.dart';
import 'rust/api/update.dart' as api;
import 'state/piece.dart';
import 'theme.dart';
import 'widgets.dart';
import 'window.dart' show quitApplication;

/// Where updates of Fogata stand, as the core says (architecture.md, 5f): asked again
/// whenever it says they changed.
class Updates extends Piece {
  Updates({this.onArrived});

  /// Called once for each newer version that becomes ready to install.
  final void Function(api.UiUpdate status)? onArrived;

  /// Null until the core runs and says.
  api.UiUpdate? status;

  /// The build last announced through [onArrived].
  int? _announced;

  /// An install was asked for and has not ended.
  bool installing = false;

  /// Reads where updates stand now.
  Future<void> refresh() async {
    try {
      take(await api.updateStatus());
    } catch (_) {}
  }

  /// Takes what the core says now.
  @visibleForTesting
  void take(api.UiUpdate? now) {
    if (now == status) return;
    status = now;
    changed();
    final build = now?.releaseBuild?.toInt();
    if (now != null && ready && build != null && build != _announced) {
      _announced = build;
      onArrived?.call(now);
    }
  }

  /// A newer version is here, checked, and can be installed.
  bool get ready => status?.state == api.UiUpdateState.ready;

  /// Installs the version that is ready and, with [quit], quits for the new one to start
  /// (the caller does otherwise: it must, at once). Null when it is under way; otherwise
  /// why it failed.
  Future<String?> install({bool quit = true}) async {
    installing = true;
    changed();
    try {
      await api.installUpdate();
    } catch (e) {
      installing = false;
      changed();
      return explain(e);
    }
    if (quit) unawaited(quitApplication());
    return null;
  }
}

/// The title bar's word that a newer version is ready: green, or amber when this one no
/// longer works with it, with a button that restarts into it. Nothing while there is
/// none.
class UpdatePill extends StatelessWidget {
  const UpdatePill({super.key, required this.updates, required this.say});
  final Updates updates;
  final void Function(String, {bool error}) say;

  @override
  Widget build(BuildContext context) => ListenableBuilder(
    listenable: updates,
    builder: (context, _) {
      final status = updates.status;
      if (!updates.ready || status == null) return const SizedBox.shrink();
      final c = context.fg;
      final version = status.releaseVersion ?? '';
      final required = status.required_;
      final (line, soft) = required
          ? (c.amberLine, c.amberSoft)
          : (c.greenLine, c.greenSoft);
      return Padding(
        padding: const EdgeInsets.only(right: 10),
        child: Tooltip(
          message: required
              ? l10n.updateRequiredTip
              : l10n.updateReadyTip(version),
          child: Container(
            key: const ValueKey('update-pill'),
            height: heightForText(context, 28, 12.5),
            // The title bar keeps room for the app's name and the page's: in large text
            // or a long language the words shorten instead.
            constraints: largeText(context)
                ? BoxConstraints(
                    maxWidth: MediaQuery.sizeOf(context).width * .4,
                  )
                : null,
            padding: const EdgeInsets.only(left: 12, right: 5),
            decoration: BoxDecoration(
              color: soft,
              borderRadius: BorderRadius.circular(999),
              border: Border.all(color: line),
            ),
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                FgIcon(Ic.download, size: 14, color: required ? c.amber : c.fg),
                const SizedBox(width: 8),
                Flexible(
                  child: Text(
                    required
                        ? l10n.updateRequired(version)
                        : l10n.updateReady(version),
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: tx(c.fg, size: 12.5, height: 1.2),
                  ),
                ),
                const SizedBox(width: 8),
                FgButton(
                  key: const ValueKey('update-restart'),
                  text: l10n.updateRestart,
                  kind: Kind.primary,
                  height: 22,
                  fontSize: 12,
                  padding: const EdgeInsets.symmetric(horizontal: 10),
                  busy: updates.installing,
                  onTap: () async {
                    final failed = await updates.install();
                    if (failed != null) {
                      say(l10n.updateFailed(failed), error: true);
                    }
                  },
                ),
              ],
            ),
          ),
        ),
      );
    },
  );
}
