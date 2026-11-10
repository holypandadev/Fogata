import 'package:flutter/material.dart';

import '../app_state.dart';
import '../call.dart';
import '../l10n.dart';
import '../theme.dart';
import '../widgets.dart';

/// Opens the choice of how this device's shared screen goes out, beside `anchor`: above
/// it when there is room, else below. A change applies at once and is kept.
Future<void> showStreamQuality(BuildContext anchor) {
  final box = anchor.findRenderObject() as RenderBox?;
  final overlay =
      Overlay.of(anchor, rootOverlay: true).context.findRenderObject()
          as RenderBox?;
  if (box == null || overlay == null || !box.hasSize) return Future.value();
  final from = box.localToGlobal(Offset.zero, ancestor: overlay) & box.size;
  final above = from.top > overlay.size.height - from.bottom;
  return showGeneralDialog<void>(
    context: anchor,
    barrierDismissible: true,
    barrierLabel: l10n.commonClose,
    barrierColor: Colors.transparent,
    transitionDuration: const Duration(milliseconds: 260),
    pageBuilder: (context, _, _) => CustomSingleChildLayout(
      delegate: _NextTo(from, above: above),
      child: const Material(
        type: MaterialType.transparency,
        child: StreamQualityCard(),
      ),
    ),
    // Grows out of the button with the controls' spring.
    transitionBuilder: (context, animation, _, child) => FadeTransition(
      opacity: animation,
      child: ScaleTransition(
        alignment: above ? Alignment.bottomCenter : Alignment.topCenter,
        scale: Tween(begin: .94, end: 1.0).animate(
          CurvedAnimation(
            parent: animation,
            curve: spring,
            reverseCurve: Curves.easeIn,
          ),
        ),
        child: child,
      ),
    ),
  );
}

/// Puts the card centred over or under the button it came from, inside the window.
class _NextTo extends SingleChildLayoutDelegate {
  _NextTo(this.from, {required this.above});
  final Rect from;
  final bool above;

  static const _gap = 8.0;
  static const _margin = 12.0;

  @override
  BoxConstraints getConstraintsForChild(BoxConstraints constraints) =>
      constraints.loosen();

  @override
  Offset getPositionForChild(Size size, Size child) {
    final x = (from.center.dx - child.width / 2).clamp(
      _margin,
      size.width - child.width - _margin,
    );
    final y = above ? from.top - _gap - child.height : from.bottom + _gap;
    return Offset(x, y.clamp(_margin, size.height - child.height - _margin));
  }

  @override
  bool shouldRelayout(_NextTo old) => old.from != from || old.above != above;
}

/// The resolution and frame rate of this device's shared screen, each a row of pills.
class StreamQualityCard extends StatelessWidget {
  const StreamQualityCard({super.key});

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = context.fg;
    final quality = state.prefs.screenQuality;
    final call = state.callState.call;
    final sharing = call?.sharing ?? false;
    final source = call?.screenLines;
    Widget row(String label, Widget choice) => Row(
      children: [
        SizedBox(
          width: 76,
          child: Text(label, style: tx(c.fg2, size: 13, weight: w500)),
        ),
        // A long choice (the source's own height, other languages) shrinks to fit.
        Flexible(
          child: FittedBox(
            fit: BoxFit.scaleDown,
            alignment: Alignment.centerLeft,
            child: choice,
          ),
        ),
      ],
    );
    return Container(
      width: 336,
      padding: const EdgeInsets.fromLTRB(16, 14, 16, 16),
      decoration: BoxDecoration(
        color: c.bg1,
        borderRadius: BorderRadius.circular(16),
        border: Border.all(color: c.border),
      ),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(l10n.streamQualityTitle, style: tx(c.fg, weight: w600)),
          const SizedBox(height: 2),
          Text(
            sharing ? l10n.streamQualityNoteSharing : l10n.streamQualityNote,
            style: tx(c.fg2, size: 12, height: 1.35),
          ),
          const SizedBox(height: 14),
          row(
            l10n.streamQualityResolution,
            Segmented(
              key: const ValueKey('stream-resolution'),
              height: 32,
              tabs: [
                for (final lines in ScreenQuality.resolutions)
                  (
                    lines != null
                        ? '${lines}p'
                        : source != null
                        ? l10n.streamQualitySourceLines(source)
                        : l10n.streamQualitySource,
                    null,
                    null,
                  ),
              ],
              selected: ScreenQuality.resolutions.indexOf(quality.lines),
              onSelect: (i) => state.setScreenQuality(
                ScreenQuality(
                  lines: ScreenQuality.resolutions[i],
                  fps: quality.fps,
                ),
              ),
            ),
          ),
          const SizedBox(height: 10),
          row(
            l10n.streamQualityFrameRate,
            Segmented(
              key: const ValueKey('stream-fps'),
              height: 32,
              tabs: [
                for (final fps in ScreenQuality.rates)
                  (l10n.streamQualityFps(fps), null, null),
              ],
              selected: ScreenQuality.rates.indexOf(quality.fps),
              onSelect: (i) => state.setScreenQuality(
                ScreenQuality(
                  lines: quality.lines,
                  fps: ScreenQuality.rates[i],
                ),
              ),
            ),
          ),
          const SizedBox(height: 12),
          Text(
            l10n.streamQualityHigherNote,
            style: tx(c.fg3, size: 12, height: 1.35),
          ),
          if (call != null && sharing) ...[
            const SizedBox(height: 12),
            Container(height: 1, color: c.border),
            const SizedBox(height: 10),
            ValueListenableBuilder<Map<String, ScreenOut>>(
              valueListenable: call.screenOut,
              builder: (context, out, _) => _GoingOut(out: out),
            ),
          ],
        ],
      ),
    );
  }
}

/// What each person watching this device's screen gets now, as measured, and why someone's
/// connection made theirs smaller than the choice above.
class _GoingOut extends StatelessWidget {
  const _GoingOut({required this.out});
  final Map<String, ScreenOut> out;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = context.fg;
    final watchers = out.entries.toList()
      ..sort(
        (a, b) => state
            .nameOf(a.key, '')
            .toLowerCase()
            .compareTo(state.nameOf(b.key, '').toLowerCase()),
      );
    final lowered = [
      for (final w in watchers)
        if (w.value.lowered) state.nameOf(w.key, l10n.streamQualitySomeone),
    ];
    final others = watchers.length > lowered.length;
    final why = switch (lowered.length) {
      0 => null,
      1 => l10n.streamQualityLoweredOne(lowered.single, others ? 'yes' : 'no'),
      _ => l10n.streamQualityLoweredMany(
        lowered.join(l10n.listSeparator),
        others ? 'yes' : 'no',
      ),
    };
    return Column(
      key: const ValueKey('stream-going-out'),
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          l10n.streamQualityGoingOut,
          style: tx(c.fg3, size: 11, weight: w500, spacing: .4),
        ),
        const SizedBox(height: 6),
        if (watchers.isEmpty)
          Text(l10n.streamQualityNobody, style: tx(c.fg3, size: 12))
        else
          for (final MapEntry(key: id, value: now) in watchers)
            Padding(
              padding: const EdgeInsets.symmetric(vertical: 3),
              child: Row(
                children: [
                  if (state.knownPerson(id) case final person?)
                    Avatar(avatar: person.avatar, size: 20)
                  else
                    const SizedBox(width: 20),
                  const SizedBox(width: 8),
                  Expanded(
                    child: Text(
                      state.nameOf(id, l10n.streamQualitySomeone),
                      overflow: TextOverflow.ellipsis,
                      style: tx(c.fg, size: 13),
                    ),
                  ),
                  Text(
                    l10n.streamQualityNow(now.height, now.fps.round()),
                    style: tx(
                      now.lowered ? c.amber : c.fg2,
                      size: 12,
                      monospace: true,
                    ),
                  ),
                ],
              ),
            ),
        if (why != null) ...[
          const SizedBox(height: 6),
          Container(
            width: double.infinity,
            padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
            decoration: BoxDecoration(
              color: c.amberSoft,
              borderRadius: BorderRadius.circular(10),
            ),
            child: Text(why, style: tx(c.amber, size: 12, height: 1.4)),
          ),
        ],
      ],
    );
  }
}
