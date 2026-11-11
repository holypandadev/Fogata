import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../app_state.dart';
import '../fair_size.dart';
import '../host.dart';
import '../icons.dart';
import '../l10n.dart';
import '../rust/api/direct.dart';
import '../theme.dart';
import '../voice.dart';
import '../widgets.dart';

/// Bar width and the gap after it, in the recording bar and in a message's card.
const _barWidth = 3.0;
const _barGap = 2.0;

/// How tall a level (0 to 255) draws, from 0 to 1: from about 36 dB under full scale,
/// where a quiet room is, to about 6 dB under it.
double _liveHeight(int level) => ((level - 100) / 130).clamp(.12, 1.0);

/// `1×`, `1.5×`, `2×`.
String speedText(double speed) =>
    '${speed == speed.roundToDouble() ? speed.toInt() : speed}×';

/// The microphone button beside the message box: one click starts recording. Dimmed in a
/// call, where the call has the microphone.
class MicButton extends StatelessWidget {
  const MicButton({super.key, required this.friend});
  final String friend;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.read(context);
    final voice = state.direct.voice;
    final c = context.fg;
    final blocked = voice.inCall;
    // Its tooltip shows above it, over the chat.
    return Tooltip(
      message: blocked ? l10n.directVoiceInCall : l10n.directVoiceRecord,
      preferBelow: false,
      excludeFromSemantics: true,
      child: Opacity(
        opacity: blocked ? .45 : 1,
        child: Springy(
          key: const ValueKey('voice-record'),
          onTap: blocked ? null : () => voice.start(friend),
          label: l10n.directVoiceRecord,
          builder: (context, hovered) => Container(
            width: 34,
            height: 34,
            alignment: Alignment.center,
            decoration: BoxDecoration(
              color: hovered && !blocked ? c.hover : null,
              shape: BoxShape.circle,
            ),
            child: FgIcon(
              Ic.mic,
              size: 17,
              color: hovered && !blocked ? c.fg : c.fg2,
            ),
          ),
        ),
      ),
    );
  }
}

/// The message box while recording: Delete (Esc), the red dot and the time, the time left
/// in the last minute, the voice's bars coming in from the right, and Send (Enter).
class RecordingBar extends StatefulWidget {
  const RecordingBar({super.key, required this.friend});
  final String friend;

  @override
  State<RecordingBar> createState() => _RecordingBarState();
}

class _RecordingBarState extends State<RecordingBar> {
  final _focus = FocusNode();

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) _focus.requestFocus();
    });
  }

  @override
  void dispose() {
    _focus.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final state = AppScope.read(context);
    final voice = state.direct.voice;
    final c = context.fg;
    final rec = voice.recording;
    final elapsed = rec?.elapsedMs ?? 0;
    final lastMinute = elapsed >= voiceLastMinuteMs;
    return Focus(
      focusNode: _focus,
      onKeyEvent: (node, event) {
        if (event is! KeyDownEvent) return KeyEventResult.ignored;
        if (event.logicalKey == LogicalKeyboardKey.escape) {
          voice.escape();
          return KeyEventResult.handled;
        }
        if (event.logicalKey == LogicalKeyboardKey.enter ||
            event.logicalKey == LogicalKeyboardKey.numpadEnter) {
          voice.send();
          return KeyEventResult.handled;
        }
        return KeyEventResult.ignored;
      },
      child: Row(
        children: [
          // Dimmed while the recording is sent: it is on its way.
          Opacity(
            opacity: voice.sending ? .45 : 1,
            child: Springy(
              key: const ValueKey('voice-delete'),
              onTap: voice.sending ? null : voice.delete,
              tooltip: l10n.directVoiceDelete,
              label: l10n.directVoiceDelete,
              builder: (context, hovered) => Container(
                width: 34,
                height: 34,
                alignment: Alignment.center,
                decoration: BoxDecoration(
                  color: hovered && !voice.sending
                      ? Color.lerp(c.hover, c.fg, .06)
                      : c.hover,
                  shape: BoxShape.circle,
                ),
                child: FgIcon(
                  Ic.trash,
                  size: 15,
                  color: hovered && !voice.sending ? c.fg : c.fg2,
                ),
              ),
            ),
          ),
          const SizedBox(width: 8),
          Container(
            width: 8,
            height: 8,
            decoration: BoxDecoration(
              color: (rec?.capturing ?? true) ? c.red : c.fg3,
              shape: BoxShape.circle,
            ),
          ),
          const SizedBox(width: 8),
          ConstrainedBox(
            constraints: const BoxConstraints(minWidth: 36),
            child: Text(
              voiceTime(elapsed),
              key: const ValueKey('voice-elapsed'),
              style: tx(c.fg, size: 13, weight: w500, monospace: true),
            ),
          ),
          if (lastMinute) ...[
            const SizedBox(width: 8),
            Container(
              height: heightForText(context, 22, 11),
              padding: const EdgeInsets.symmetric(horizontal: 8),
              alignment: Alignment.center,
              decoration: BoxDecoration(
                color: c.amberSoft,
                borderRadius: BorderRadius.circular(999),
                border: Border.all(color: c.amberLine),
              ),
              child: Text(
                l10n.directVoiceLeft(
                  voiceTime(math.max(0, voiceMaxMs - elapsed) + 999),
                ),
                style: tx(c.amber, size: 11, monospace: true, height: 1.2),
              ),
            ),
          ],
          Expanded(
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 14),
              child: SizedBox(
                height: 34,
                child: CustomPaint(
                  painter: _LiveBars(
                    bars: rec?.bars ?? const [],
                    bar: c.fg,
                    dot: c.borderStrong,
                    fade: c.bg2,
                  ),
                ),
              ),
            ),
          ),
          Springy(
            key: const ValueKey('voice-send'),
            onTap: voice.sending ? null : voice.send,
            tooltip: l10n.commonSend,
            label: l10n.commonSend,
            builder: (context, hovered) => Container(
              width: 34,
              height: 34,
              alignment: Alignment.center,
              decoration: BoxDecoration(
                color: hovered ? c.pri.withValues(alpha: .88) : c.pri,
                shape: BoxShape.circle,
              ),
              child: voice.sending
                  ? Spinner(size: 14, color: c.onPri)
                  : FgIcon(Ic.send, size: 14, stroke: 2.4, color: c.onPri),
            ),
          ),
        ],
      ),
    );
  }
}

/// The voice's bars as they come, from the right, over a line of dots, fading out on the
/// left.
class _LiveBars extends CustomPainter {
  _LiveBars({
    required this.bars,
    required this.bar,
    required this.dot,
    required this.fade,
  });
  final List<int> bars;
  final Color bar, dot, fade;

  @override
  void paint(Canvas canvas, Size size) {
    const step = _barWidth + _barGap;
    final fits = ((size.width + _barGap) / step).floor();
    final mid = size.height / 2;
    final paint = Paint();
    final shown = bars.length > fits ? bars.sublist(bars.length - fits) : bars;
    for (var i = 0; i < fits; i++) {
      final x = size.width - (fits - i) * step + _barGap;
      final at = i - (fits - shown.length);
      final double h = at >= 0
          ? math.max(3, (_liveHeight(shown[at]) * 26).roundToDouble())
          : 3;
      paint.color = at >= 0 ? bar : dot;
      canvas.drawRRect(
        RRect.fromRectAndRadius(
          Rect.fromLTWH(x, mid - h / 2, _barWidth, h),
          const Radius.circular(2),
        ),
        paint,
      );
    }
    // The oldest bars fade into the box.
    final rect = Rect.fromLTWH(0, 0, math.min(60, size.width), size.height);
    canvas.drawRect(
      rect,
      Paint()
        ..shader = LinearGradient(colors: [fade, fade.withValues(alpha: 0)])
            .createShader(rect),
    );
  }

  @override
  bool shouldRepaint(_LiveBars old) =>
      !identical(old.bars, bars) ||
      old.bar != bar ||
      old.dot != dot ||
      old.fade != fade;
}

/// The line under the recording bar: which microphone records, and what Esc and Enter do;
/// once Esc no longer deletes the recording ([escDeletes]), only what Enter does.
class RecordingUnder extends StatelessWidget {
  const RecordingUnder({
    super.key,
    required this.microphone,
    this.fellBack = false,
    this.escDeletes = true,
  });
  final String microphone;

  /// The microphone chosen could not be used: the system's default records.
  final bool fellBack;

  final bool escDeletes;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    const esc = '\u{E000}', enter = '\u{E001}';
    final keys = escDeletes
        ? l10n.directVoiceKeys(esc, enter)
        : l10n.directVoiceKeySend(enter);
    final keyStyle = tx(c.fg3, size: 11, monospace: true);
    final spans = <InlineSpan>[
      TextSpan(
        text:
            '${fellBack ? l10n.directVoiceRecordingDefault : l10n.directVoiceRecordingFrom(microphone)} · ',
      ),
    ];
    for (final part in keys.split(
      RegExp('(?=[$esc$enter])|(?<=[$esc$enter])'),
    )) {
      spans.add(switch (part) {
        esc => TextSpan(text: 'Esc', style: keyStyle),
        enter => TextSpan(text: 'Enter', style: keyStyle),
        _ => TextSpan(text: part),
      });
    }
    return Text.rich(
      TextSpan(children: spans),
      key: const ValueKey('voice-under'),
      maxLines: 1,
      overflow: TextOverflow.ellipsis,
      style: tx(c.fg3, size: 11),
    );
  }
}

/// Over the message box when no microphone could be used: what to check, a way to the
/// settings of the microphone, and a button to close it. Over the recording bar when the
/// microphone stopped part way ([stopped]): what was recorded can still be sent, and the
/// note goes with the recording.
class MicProblemNote extends StatelessWidget {
  const MicProblemNote({super.key, this.stopped = false});
  final bool stopped;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.read(context);
    final c = context.fg;
    return Container(
      key: ValueKey(stopped ? 'voice-mic-stopped' : 'voice-no-mic'),
      margin: const EdgeInsets.only(bottom: 8),
      padding: const EdgeInsets.fromLTRB(14, 8, 8, 8),
      decoration: BoxDecoration(
        color: c.amberSoft,
        borderRadius: BorderRadius.circular(16),
        border: Border.all(color: c.amberLine),
      ),
      // The words beside the button while there is room for them, over it when large
      // text or a narrow chat leaves too little.
      child: LayoutBuilder(
        builder: (context, box) {
          final under =
              largeText(context) &&
              box.maxWidth < MediaQuery.textScalerOf(context).scale(440);
          final words = Text.rich(
            TextSpan(
              children: [
                TextSpan(
                  text: stopped
                      ? l10n.directVoiceMicStopped
                      : l10n.directVoiceNoMic,
                  style: tx(c.fg, size: 13, weight: w500, height: 1.4),
                ),
                if (!stopped)
                  TextSpan(
                    text:
                        ' ${l10n.directVoiceNoMicCheck(onMacOS ? 'macos' : 'other')}',
                  ),
              ],
            ),
            style: tx(c.fg2, size: 13, height: 1.4),
          );
          final settings = FgButton(
            key: const ValueKey('voice-settings'),
            text: l10n.devicesVoiceSettings,
            trailing: Ic.arrowRight,
            height: 28,
            fontSize: 12,
            padding: const EdgeInsets.symmetric(horizontal: 10),
            onTap: () {
              state.direct.voice.closeMicProblem();
              state.openSettings(SettingsSection.voice);
            },
          );
          return Row(
            crossAxisAlignment: under
                ? CrossAxisAlignment.start
                : CrossAxisAlignment.center,
            children: [
              FgIcon(Ic.micOff, size: 16, color: c.amber),
              const SizedBox(width: 10),
              Expanded(
                child: under
                    ? Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [words, const SizedBox(height: 6), settings],
                      )
                    : Row(
                        children: [
                          Expanded(child: words),
                          const SizedBox(width: 10),
                          settings,
                        ],
                      ),
              ),
              if (!stopped) ...[
                const SizedBox(width: 4),
                FgButton(
                  icon: Ic.close,
                  kind: Kind.ghost,
                  height: 28,
                  padding: const EdgeInsets.symmetric(horizontal: 7),
                  tooltip: l10n.commonClose,
                  onTap: state.direct.voice.closeMicProblem,
                ),
              ],
            ],
          );
        },
      ),
    );
  }
}

/// A voice message in the chat: round play or pause, its bars (the part played lit), its
/// time (counting while it plays), and the speed it plays at. A friend's not played yet
/// has a dot. One not here yet waits, dimmed; one that could not come has Download.
class VoiceCard extends StatelessWidget {
  const VoiceCard({
    super.key,
    required this.friend,
    required this.file,
    required this.mine,
    required this.canFetch,
    required this.onFetch,
    this.notHere,
  });
  final String friend;
  final UiDirectFile file;
  final bool mine;

  /// When it can come, for one not here that cannot come now: `Available when Marta is
  /// online`.
  final String? notHere;

  /// Both are online: a message not here can come now.
  final bool canFetch;
  final VoidCallback onFetch;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.read(context);
    final voice = state.direct.voice;
    final info = file.voice!;
    return ListenableBuilder(
      listenable: voice,
      builder: (context, _) {
        final c = context.fg;
        final here = file.state == UiFileState.here;
        final failed = !here && file.state != UiFileState.coming && canFetch;
        final playing = voice.playing(file.id);
        // Its sender's word, checked by the core; drawn within what can be shown.
        final duration = fairLength(info.durationMs);
        // Played to the end, it shows as it did before it played.
        final at = here ? voice.positionOf(file.id) : null;
        final position = at != null && (playing || at < duration - 50)
            ? at
            : null;
        final played = position == null || duration <= 0
            ? 0.0
            : (position / duration).clamp(0.0, 1.0);
        final speed = voice.speedOf(file.id);
        final bars = failed ? _squeeze(info.bars, 30) : info.bars.toList();
        final card = Semantics(
          label: l10n.directVoiceMessage,
          child: Container(
            key: ValueKey('voice-card:${file.id}'),
            width: 372,
            padding: const EdgeInsets.fromLTRB(8, 8, 10, 8),
            decoration: BoxDecoration(
              color: c.bg1,
              borderRadius: BorderRadius.circular(14),
              border: Border.all(color: c.border),
            ),
            child: Row(
              children: [
                Opacity(
                  opacity: here ? 1 : .45,
                  child: Springy(
                    key: ValueKey('voice-play:${file.id}'),
                    onTap: here ? () => voice.toggle(friend, file) : null,
                    tooltip: playing ? l10n.directPause : l10n.directPlay,
                    label: playing ? l10n.directPause : l10n.directPlay,
                    builder: (context, hovered) => Container(
                      width: 34,
                      height: 34,
                      alignment: Alignment.center,
                      decoration: BoxDecoration(
                        color: hovered && here
                            ? c.pri.withValues(alpha: .88)
                            : c.pri,
                        shape: BoxShape.circle,
                      ),
                      child: FgIcon(
                        playing ? Ic.pause : Ic.play,
                        size: 14,
                        color: c.onPri,
                      ),
                    ),
                  ),
                ),
                // The bars, the time and the dot from the left, the speed on the right; as
                // many bars as fit, since longer words around them leave fewer.
                Expanded(
                  child: Row(
                    children: [
                      Flexible(
                        child: Padding(
                          padding: const EdgeInsets.symmetric(horizontal: 12),
                          child: SizedBox(
                            width:
                                bars.length * (_barWidth + _barGap) - _barGap,
                            height: 28,
                            child: CustomPaint(
                              painter: _CardBars(
                                bars: bars,
                                played: played,
                                lit: c.fg,
                                rest: here ? c.borderStrong : c.border,
                              ),
                            ),
                          ),
                        ),
                      ),
                      ConstrainedBox(
                        constraints: const BoxConstraints(minWidth: 34),
                        child: Text(
                          voiceTime(position ?? duration),
                          key: ValueKey('voice-time:${file.id}'),
                          style: tx(
                            playing ? c.fg : c.fg2,
                            size: 12,
                            monospace: true,
                            height: 1.2,
                          ),
                        ),
                      ),
                      if (!mine && !info.played && here)
                        Tooltip(
                          message: l10n.directVoiceNotPlayed,
                          child: Container(
                            key: ValueKey('voice-new:${file.id}'),
                            width: 6,
                            height: 6,
                            margin: const EdgeInsets.only(right: 6),
                            decoration: BoxDecoration(
                              color: c.blue,
                              shape: BoxShape.circle,
                            ),
                          ),
                        ),
                    ],
                  ),
                ),
                const SizedBox(width: 8),
                if (failed)
                  FgButton(
                    key: ValueKey('direct-get:${file.id}'),
                    text: l10n.commonDownload,
                    icon: Ic.download,
                    height: 28,
                    fontSize: 12,
                    padding: const EdgeInsets.symmetric(horizontal: 10),
                    onTap: onFetch,
                  )
                else if (here)
                  Springy(
                    key: ValueKey('voice-speed:${file.id}'),
                    onTap: () => voice.nextSpeed(file.id),
                    tooltip: l10n.directVoiceSpeed,
                    label: l10n.directVoiceSpeed,
                    builder: (context, hovered) => Container(
                      height: heightForText(context, 22, 11),
                      constraints: const BoxConstraints(minWidth: 34),
                      padding: const EdgeInsets.symmetric(horizontal: 7),
                      alignment: Alignment.center,
                      decoration: BoxDecoration(
                        color: speed != 1 || hovered ? c.raised : c.bg2,
                        borderRadius: BorderRadius.circular(999),
                        border: Border.all(color: c.border),
                      ),
                      child: Text(
                        speedText(speed),
                        style: tx(
                          speed != 1 ? c.fg : c.fg2,
                          size: 11,
                          monospace: true,
                          height: 1.2,
                        ),
                      ),
                    ),
                  ),
              ],
            ),
          ),
        );
        final why = notHere;
        if (why == null) return card;
        return Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            card,
            const SizedBox(height: 4),
            Row(
              children: [
                FgIcon(Ic.clock, size: 12, color: c.fg3),
                const SizedBox(width: 6),
                Flexible(
                  child: Text(why, style: tx(c.fg3, size: 11, height: 1.3)),
                ),
              ],
            ),
          ],
        );
      },
    );
  }

  /// `levels` as `n` bars, the loudest of each stretch.
  static List<int> _squeeze(List<int> levels, int n) => [
    for (var i = 0; i < n; i++)
      levels
          .sublist(
            i * levels.length ~/ n,
            math.max(i * levels.length ~/ n + 1, (i + 1) * levels.length ~/ n),
          )
          .fold(0, math.max),
  ];
}

/// A message's bars: the part played lit, the rest dim.
class _CardBars extends CustomPainter {
  _CardBars({
    required this.bars,
    required this.played,
    required this.lit,
    required this.rest,
  });
  final List<int> bars;
  final double played;
  final Color lit, rest;

  @override
  void paint(Canvas canvas, Size size) {
    final paint = Paint();
    final mid = size.height / 2;
    final fits = ((size.width + _barGap) / (_barWidth + _barGap)).floor();
    final bars = fits < this.bars.length
        ? VoiceCard._squeeze(this.bars, math.max(1, fits))
        : this.bars;
    final litUpTo = (played * bars.length).round();
    for (var i = 0; i < bars.length; i++) {
      final h = math.max(3.0, (bars[i] / 255 * size.height).roundToDouble());
      paint.color = i < litUpTo ? lit : rest;
      canvas.drawRRect(
        RRect.fromRectAndRadius(
          Rect.fromLTWH(i * (_barWidth + _barGap), mid - h / 2, _barWidth, h),
          const Radius.circular(2),
        ),
        paint,
      );
    }
  }

  @override
  bool shouldRepaint(_CardBars old) =>
      old.played != played ||
      !identical(old.bars, bars) ||
      old.lit != lit ||
      old.rest != rest;
}
