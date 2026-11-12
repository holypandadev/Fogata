import 'dart:math' as math;

import 'package:flutter/widgets.dart';

import 'l10n.dart';
import 'motion.dart';
import 'theme.dart';

/// Who is typing in each chat, from the signals that come: each person shows until a few
/// seconds after their last one, until they say they stopped, their message arrives, or
/// they go. Nothing of it is kept.
class TypingBoard {
  /// How long someone shows after their last signal.
  static const showFor = Duration(seconds: 6);

  /// A direct chat's key: the friend's id.
  static String direct(String friend) => 'dm:$friend';

  /// A room's chat's key: its group and voice room.
  static String room(String group, String voice) => 'room:$group:$voice';

  /// By chat, who is typing and until when (ms), in the order they started.
  final Map<String, Map<String, int>> _chats = {};

  /// `device` is typing in `chat` now.
  void heard(String chat, String device, int nowMs) {
    _chats.putIfAbsent(chat, () => {})[device] = nowMs + showFor.inMilliseconds;
  }

  /// `device` stopped typing in `chat`: it stopped, or its message arrived.
  void stopped(String chat, String device) {
    final who = _chats[chat];
    if (who == null) return;
    who.remove(device);
    if (who.isEmpty) _chats.remove(chat);
  }

  /// `device` went: it shows nowhere.
  void gone(String device) {
    for (final chat in _chats.keys.toList()) {
      stopped(chat, device);
    }
  }

  /// Nobody shows in `chat` any more (the room closed).
  void clear(String chat) => _chats.remove(chat);

  /// Nobody shows anywhere (the core stopped).
  void clearAll() => _chats.clear();

  /// Who is typing in `chat` at `nowMs`, in the order they started.
  List<String> who(String chat, int nowMs) => [
    for (final MapEntry(key: device, value: until)
        in _chats[chat]?.entries ?? const <MapEntry<String, int>>[])
      if (until > nowMs) device,
  ];

  /// Drops those whose time ran out; whether anyone went.
  bool prune(int nowMs) {
    var changed = false;
    for (final chat in _chats.keys.toList()) {
      final who = _chats[chat]!;
      final before = who.length;
      who.removeWhere((_, until) => until <= nowMs);
      if (who.length != before) changed = true;
      if (who.isEmpty) _chats.remove(chat);
    }
    return changed;
  }

  /// When the next person's time runs out, if anyone shows.
  int? get nextEnd {
    int? next;
    for (final who in _chats.values) {
      for (final until in who.values) {
        if (next == null || until < next) next = until;
      }
    }
    return next;
  }

  bool get isEmpty => _chats.isEmpty;
}

/// Tells a chat that this device is typing while the text in its message box changes: at
/// most once every [every], never for an empty box, and "stopped" when the box empties
/// (the message went, or it was cleared) or closes.
class TypingSender {
  TypingSender(this._send);

  final void Function(bool typing) _send;

  /// The others show someone for [TypingBoard.showFor] after their last signal.
  static const every = Duration(seconds: 3);

  String _text = '';
  int? _lastMs;

  /// The text in the box now.
  void changed(String text, {int? nowMs}) {
    if (text == _text) return;
    _text = text;
    if (text.trim().isEmpty) {
      stopped();
      return;
    }
    final now = nowMs ?? DateTime.now().millisecondsSinceEpoch;
    final last = _lastMs;
    if (last != null && now - last < every.inMilliseconds) return;
    _lastMs = now;
    _send(true);
  }

  /// The box emptied or closes: "stopped", if "typing" went since the last one.
  void stopped() {
    if (_lastMs == null) return;
    _lastMs = null;
    _send(false);
  }
}

/// "Ana is typing…", "Ana and Leo are typing…", or "Several people are typing…"; the
/// names in [bold].
TextSpan typingText(List<String> names, TextStyle bold) {
  const first = '\u{E000}', second = '\u{E001}';
  return switch (names.length) {
    1 => TextSpan(
      children: styledSpans(l10n.typingOne(first), {
        first: TextSpan(text: names[0], style: bold),
      }),
    ),
    2 => TextSpan(
      children: styledSpans(l10n.typingTwo(first, second), {
        first: TextSpan(text: names[0], style: bold),
        second: TextSpan(text: names[1], style: bold),
      }),
    ),
    _ => TextSpan(text: l10n.typingSeveral),
  };
}

/// A message as spans, with its placeholders drawn their own way: [text] was said with
/// the keys of [parts] in place of some placeholders, and each becomes its span, wherever
/// the language put it.
List<InlineSpan> styledSpans(String text, Map<String, InlineSpan> parts) {
  final out = <InlineSpan>[];
  var rest = text;
  while (rest.isNotEmpty) {
    var at = -1;
    String? found;
    for (final key in parts.keys) {
      final i = rest.indexOf(key);
      if (i >= 0 && (at < 0 || i < at)) {
        at = i;
        found = key;
      }
    }
    if (found == null) {
      out.add(TextSpan(text: rest));
      break;
    }
    if (at > 0) out.add(TextSpan(text: rest.substring(0, at)));
    out.add(parts[found]!);
    rest = rest.substring(at + found.length);
  }
  return out;
}

/// The line under a chat's messages that says who is typing, with three small dots that
/// rise and fade in turn. Always [height] tall, so the messages never jump; empty while
/// nobody types.
class TypingLine extends StatelessWidget {
  const TypingLine({
    super.key,
    required this.names,
    this.padding = EdgeInsets.zero,
  });

  final List<String> names;
  final EdgeInsets padding;
  static const height = 20.0;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final shown = names.isNotEmpty;
    final text = typingText(
      names,
      tx(c.fg2, size: 12, weight: w600, height: 1.2),
    );
    return SizedBox(
      height: height,
      child: Padding(
        padding: padding,
        child: AnimatedOpacity(
          opacity: shown ? 1 : 0,
          duration: const Duration(milliseconds: 160),
          child: !shown
              ? const SizedBox.shrink()
              : Semantics(
                  liveRegion: true,
                  label: text.toPlainText(),
                  excludeSemantics: true,
                  child: Row(
                    children: [
                      const TypingDots(),
                      const SizedBox(width: 6),
                      Flexible(
                        child: Text.rich(
                          text,
                          key: const ValueKey('typing-line'),
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: tx(c.fg3, size: 12, height: 1.2),
                        ),
                      ),
                    ],
                  ),
                ),
        ),
      ),
    );
  }
}

/// Three dots that rise a little and brighten one after the other.
class TypingDots extends StatefulWidget {
  const TypingDots({super.key});

  @override
  State<TypingDots> createState() => _TypingDotsState();
}

class _TypingDotsState extends State<TypingDots>
    with SingleTickerProviderStateMixin {
  late final _c = AnimationController(
    vsync: this,
    duration: const Duration(milliseconds: 1200),
  );

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    // With less motion, the dots hold still, at rest.
    if (lessMotion(context)) {
      _c.value = .9;
    } else if (!_c.isAnimating) {
      _c.repeat();
    }
  }

  @override
  void dispose() {
    _c.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    return RepaintBoundary(
      child: AnimatedBuilder(
        animation: _c,
        builder: (context, _) => Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            for (var i = 0; i < 3; i++) ...[
              if (i > 0) const SizedBox(width: 3),
              _dot(c, _wave(_c.value - i * .15)),
            ],
          ],
        ),
      ),
    );
  }

  /// 0 at rest, 1 at the top: each dot rises in the first part of the cycle.
  static double _wave(double t) {
    final at = t % 1;
    if (at > .45) return 0;
    return math.sin(at / .45 * math.pi);
  }

  Widget _dot(Fg c, double up) => Transform.translate(
    offset: Offset(0, -2 * up),
    child: Container(
      width: 4,
      height: 4,
      decoration: BoxDecoration(
        shape: BoxShape.circle,
        color: Color.lerp(c.fg3.withValues(alpha: .45), c.fg2, up),
      ),
    ),
  );
}
