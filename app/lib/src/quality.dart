import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_webrtc/flutter_webrtc.dart' show StatsReport;
import 'package:intl/intl.dart' show NumberFormat;

import 'app_state.dart';
import 'format.dart';
import 'l10n.dart';
import 'rust/api/types.dart';
import 'screens/room.dart' show pathWords;
import 'theme.dart';
import 'widgets.dart';

/// How well a connection carries a call, from best to worst, with how it is drawn: the
/// bars lit, their colour and the colour behind its word. The names are what devices
/// send each other, so [weak] keeps its name while it shows as Poor.
enum NetLevel {
  good(4, Color(0xFF3DD68C), Color.fromRGBO(61, 214, 140, .12)),
  okay(2, Color(0xFFF5A524), Color.fromRGBO(245, 165, 36, .14)),
  weak(1, Color(0xFFFF6166), Color.fromRGBO(255, 97, 102, .14)),
  lost(0, Color(0xFFA1A1A1), Color.fromRGBO(255, 255, 255, .08));

  const NetLevel(this.lit, this.color, this.soft);

  /// The level in a word: Good, Okay, Poor, Lost.
  String get word => switch (this) {
    good => l10n.qualityGood,
    okay => l10n.qualityOkay,
    weak => l10n.qualityPoor,
    lost => l10n.qualityLost,
  };

  final int lit;
  final Color color;
  final Color soft;

  NetLevel worse(NetLevel other) => other.index > index ? other : this;
  NetLevel better(NetLevel other) => other.index < index ? other : this;

  /// Poor or nothing at all: someone may be heard choppy.
  bool get bad => index >= weak.index;

  /// The level from the round trip and the share of packets lost: good under 150 ms and
  /// 2 %, okay under 300 ms and 5 %, poor past either. A figure not known yet counts as
  /// fine.
  static NetLevel of({int? delayMs, double? lossPercent}) {
    final delay = delayMs ?? 0;
    final loss = lossPercent ?? 0;
    if (delay < 150 && loss < 2) return good;
    if (delay < 300 && loss < 5) return okay;
    return weak;
  }

  /// The level another device names in what it sends; null for none or one unknown here.
  static NetLevel? named(Object? name) =>
      values.where((l) => l.name == name).firstOrNull;
}

/// How long nothing may arrive from someone before they count as lost.
const lostAfter = Duration(seconds: 3);

/// Keeps a level from flickering: a new level shows only after two readings in a row
/// give it. Losing someone shows at once, since [lostAfter] has already passed, and so
/// does their coming back.
class LevelGate {
  NetLevel? level;
  NetLevel? _next;

  /// Takes a reading; true when the level shown changed.
  bool take(NetLevel reading) {
    final shown = level;
    if (shown == reading) {
      _next = null;
      return false;
    }
    if (shown == null || shown == NetLevel.lost || reading == NetLevel.lost) {
      level = reading;
      _next = null;
      return true;
    }
    if (_next == reading) {
      level = reading;
      _next = null;
      return true;
    }
    _next = reading;
    return false;
  }
}

/// A value of a connection's chosen path, from its statistics: the candidate pair the
/// transport selected, or else the nominated one.
num? _ofPath(Iterable<StatsReport> reports, String key) {
  final selected = reports
      .where((r) => r.type == 'transport')
      .map((r) => r.values['selectedCandidatePairId'])
      .whereType<String>()
      .firstOrNull;
  for (final r in reports) {
    if (r.type != 'candidate-pair') continue;
    final chosen = selected != null
        ? r.id == selected
        : r.values['nominated'] == true;
    final value = r.values[key];
    if (chosen && value is num) return value;
  }
  return null;
}

/// The round trip of a connection's chosen path in milliseconds.
int? roundTripMs(Iterable<StatsReport> reports) {
  final rtt = _ofPath(reports, 'currentRoundTripTime');
  return rtt == null ? null : (rtt * 1000).round();
}

/// What WebRTC estimates a connection's chosen path can send now, in kbit/s.
double? availableKbps(Iterable<StatsReport> reports) {
  final bps = _ofPath(reports, 'availableOutgoingBitrate');
  return bps == null ? null : bps / 1000;
}

/// One reading of a connection's statistics, kept to what tells its quality.
@immutable
class LinkSample {
  const LinkSample({
    this.delayMs,
    this.heard = 0,
    this.voice = (0, 0),
    this.video = (0, 0),
  });

  /// Round trip in milliseconds.
  final int? delayMs;

  /// A count that changes whenever anything arrives: the connectivity checks and their
  /// answers, which keep coming every few seconds even when nobody sends sound or
  /// pictures, and the bytes of what it carries.
  final int heard;

  /// Packets of the voice lost and received so far, and of the pictures.
  final (int, int) voice;
  final (int, int) video;

  /// Reads [reports]; the voice is the channel with mid [voiceMid].
  factory LinkSample.of(
    Iterable<StatsReport> reports, {
    String voiceMid = '0',
  }) {
    var heard = 0;
    var voice = (0, 0);
    var video = (0, 0);
    for (final r in reports) {
      final v = r.values;
      int n(String key) => math.max(0, (v[key] as num?)?.toInt() ?? 0);
      if (r.type == 'candidate-pair') {
        heard +=
            n('requestsReceived') + n('responsesReceived') + n('bytesReceived');
      } else if (r.type == 'inbound-rtp') {
        final counts = (n('packetsLost'), n('packetsReceived'));
        final mid = v['mid'];
        if (v['kind'] == 'audio' && (mid == null || '$mid' == voiceMid)) {
          voice = (voice.$1 + counts.$1, voice.$2 + counts.$2);
        } else if (v['kind'] == 'video') {
          video = (video.$1 + counts.$1, video.$2 + counts.$2);
        }
      }
    }
    return LinkSample(
      delayMs: roundTripMs(reports),
      heard: heard,
      voice: voice,
      video: video,
    );
  }
}

/// The share of packets lost over the last stretch with enough of them to tell: a quiet
/// voice sends a few packets a second, and one lost among a handful would read as a
/// large loss.
class _Losses {
  (int, int, DateTime)? _base;
  double? percent;
  DateTime? _at;

  void take((int, int) counts, DateTime now) {
    final (lost, received) = counts;
    final base = _base;
    // A new connection counts from zero again.
    if (base == null || lost < base.$1 || received < base.$2) {
      _base = (lost, received, now);
      return;
    }
    final newLost = lost - base.$1;
    final total = newLost + received - base.$2;
    final age = now.difference(base.$3);
    if (total >= 50 || (total >= 20 && age >= const Duration(seconds: 6))) {
      percent = newLost * 100 / total;
      _at = now;
      _base = (lost, received, now);
    } else if (age >= const Duration(seconds: 10)) {
      _base = (lost, received, now);
    }
    final at = _at;
    if (at != null && now.difference(at) > const Duration(seconds: 12)) {
      percent = null;
    }
  }

  /// Counts afresh: what went missing while nothing arrived is not a share of what does.
  void restart() {
    _base = null;
    percent = null;
    _at = null;
  }
}

/// What is known about the connection to one other participant.
class _Link {
  final gate = LevelGate();
  int? delayMs;
  final _voice = _Losses();
  final _video = _Losses();
  int? _heard;
  DateTime? heardAt;

  /// Their own network, as they last said.
  NetLevel? theirs;

  double? get lossPercent => _voice.percent ?? _video.percent;
}

/// The quality of a call's connections: to each other participant as this device measures
/// it, and each one's own network as they say. It notifies only when a level shown
/// changes; [readings] ticks with every reading, for what shows the figures.
class CallQuality extends ChangeNotifier {
  CallQuality({required this.me});

  /// This device's id.
  final String me;
  final Map<String, _Link> _links = {};
  final readings = ValueNotifier<int>(0);

  /// The connection to [id], once measured.
  NetLevel? linkOf(String id) => _links[id]?.gate.level;

  /// [id]'s own network as they said it, if they did.
  NetLevel? theirsOf(String id) => _links[id]?.theirs;

  int? delayOf(String id) => _links[id]?.delayMs;
  double? lossOf(String id) => _links[id]?.lossPercent;

  /// How long nothing has come from [id], at the last reading.
  Duration? silenceOf(String id, DateTime now) {
    final at = _links[id]?.heardAt;
    return at == null ? null : now.difference(at);
  }

  /// The others with a measured connection.
  Iterable<String> get measured =>
      _links.entries.where((e) => e.value.gate.level != null).map((e) => e.key);

  /// This device's own network: if any of its connections is good, its network can carry
  /// a call, so the best of them; null until one is measured.
  NetLevel? get mine {
    NetLevel? best;
    for (final id in measured) {
      final level = linkOf(id)!;
      best = best == null ? level : best.better(level);
    }
    return best;
  }

  /// [mine] when it tells the network apart from the connections: with two or more
  /// others measured. With one, a bad connection may be on either side.
  NetLevel? get network => measured.length >= 2 ? mine : null;

  /// What shows for [id]: for this device its own network, for the others the worse of
  /// the connection and their own network.
  NetLevel? shownOf(String id) {
    if (id == me) return mine;
    final link = linkOf(id);
    final theirs = theirsOf(id);
    return link == null || theirs == null ? link : link.worse(theirs);
  }

  /// Takes a reading of the connection to [id] made at [now].
  void take(String id, LinkSample sample, DateTime now) {
    final link = _links.putIfAbsent(id, _Link.new);
    if (link._heard != sample.heard || link.heardAt == null) {
      link._heard = sample.heard;
      link.heardAt = now;
      if (link.gate.level == NetLevel.lost) {
        link._voice.restart();
        link._video.restart();
      }
    }
    link.delayMs = sample.delayMs ?? link.delayMs;
    link._voice.take(sample.voice, now);
    link._video.take(sample.video, now);
    final silent = now.difference(link.heardAt!) >= lostAfter;
    _read(
      link,
      silent
          ? NetLevel.lost
          : NetLevel.of(delayMs: link.delayMs, lossPercent: link.lossPercent),
    );
  }

  /// The connection to [id] is down. Someone never reached has no level to lose.
  void cut(String id) {
    final link = _links[id];
    if (link == null || link.gate.level == null) return;
    _read(link, NetLevel.lost);
  }

  void _read(_Link link, NetLevel reading) {
    final changed = link.gate.take(reading);
    readings.value++;
    if (changed) notifyListeners();
  }

  /// [id] says how its own network is; null when it cannot tell.
  void theirs(String id, NetLevel? level) {
    final link = _links.putIfAbsent(id, _Link.new);
    if (link.theirs == level) return;
    link.theirs = level;
    notifyListeners();
  }

  /// Forgets [id]'s connection: it has none, and is not trying to reconnect.
  void forget(String id) {
    if (_links.remove(id) != null) notifyListeners();
  }

  /// Forgets those no longer in the call.
  void keep(Set<String> ids) {
    final before = _links.length;
    _links.removeWhere((id, _) => !ids.contains(id));
    if (_links.length != before) notifyListeners();
  }

  @override
  void dispose() {
    readings.dispose();
    super.dispose();
  }
}

// ----- Words -----

/// A delay as shown: `32 ms`.
String delayText(int ms) => l10n.qualityDelay(ms);

/// A share of packets as shown: `0.1 %`, `9 %`, `12 %`.
String lossText(double percent) {
  if (percent >= 10) return l10n.qualityPercent('${percent.round()}');
  return l10n.qualityPercent(NumberFormat('0.#', intlLocale).format(percent));
}

/// What the details of [name]'s connection say about it, in a sentence: whose side the
/// trouble is on, if any. [link] is the connection to them, [theirs] their own network as
/// they say and [mine] this device's own network, when known; [silence] how long nothing
/// has come from them. [through] names the member that carries them, when known.
String hintAbout({
  required String name,
  required NetLevel link,
  NetLevel? theirs,
  NetLevel? mine,
  bool throughMember = false,
  String? through,
  Duration silence = Duration.zero,
}) {
  final carrier = through ?? l10n.qualityAnotherMember;
  if (link == NetLevel.lost) {
    return l10n.qualityLostHint(name, silence.inSeconds);
  }
  final theyBad = theirs != null && theirs.bad;
  final iBad = mine != null && mine.bad;
  if (theyBad && iBad) {
    return l10n.qualityBothPoor(name);
  }
  if (theyBad) {
    return l10n.qualityTheirSide(name);
  }
  if (iBad) {
    return l10n.qualityYourSideWith(name);
  }
  if (theirs == NetLevel.okay) {
    return l10n.qualityTheirsSlow(name);
  }
  if (link == NetLevel.good) {
    return l10n.qualityAllGoodWith(name);
  }
  if (mine == NetLevel.okay) {
    return l10n.qualityMineSlowWith(name);
  }
  if (theirs == NetLevel.good) {
    return throughMember
        ? l10n.qualityWayThrough(name, carrier)
        : l10n.qualityWaySlowFine(name);
  }
  return throughMember
      ? l10n.qualityWayVia(link.name, name, carrier)
      : l10n.qualityWay(link.name, name);
}

/// Another participant as the details of this device's own connection mention them;
/// `through` names the member that carries them, when known.
typedef OtherLink = ({
  String name,
  NetLevel link,
  NetLevel? theirs,
  bool throughMember,
  String? through,
  Duration silence,
});

/// What the details of this device's own connection say: whether the trouble is on its
/// side, and if not, who has trouble. [network] is null when it cannot be told apart from
/// the connections (one other participant).
String hintAboutMe({NetLevel? network, required List<OtherLink> others}) {
  if (network != null && network.bad) {
    return l10n.qualityYourSide;
  }
  if (network == NetLevel.okay) {
    return l10n.qualityMineSlow;
  }
  if (network == null && others.length == 1) {
    final o = others.single;
    if (o.link == NetLevel.lost) {
      return l10n.qualityLostShort(o.name, o.silence.inSeconds);
    }
    if (o.link == NetLevel.good) {
      return l10n.qualityAllWellWith(o.name);
    }
    return l10n.qualityWay(o.link.name, o.name);
  }
  final notes = [
    for (final o in others)
      if (o.link == NetLevel.lost)
        l10n.qualityNoteDropped(o.name)
      else if (o.theirs != null && o.theirs!.bad)
        l10n.qualityNoteTheirsPoor(o.name)
      else if (o.theirs == NetLevel.okay)
        l10n.qualityNoteTheirsSlow(o.name)
      else if (o.link != NetLevel.good && o.throughMember)
        l10n.qualityNoteThrough(o.name, o.through ?? l10n.qualityAnotherMember)
      else if (o.link != NetLevel.good)
        l10n.qualityNoteWay(o.link.name, o.name),
  ];
  if (notes.isEmpty) return l10n.qualityAllWell;
  return l10n.qualityAllWellMine(
    capitalized(notes.join(l10n.qualityNoteSeparator)),
  );
}

// ----- Drawing -----

/// The colours of a level on the window's surfaces (not over a picture), in the theme.
Color _levelColor(Fg c, NetLevel level) => c.dark
    ? level.color
    : switch (level) {
        NetLevel.good => c.green,
        NetLevel.okay => c.amber,
        NetLevel.weak => c.red,
        NetLevel.lost => c.fg2,
      };

Color _levelSoft(Fg c, NetLevel level) => c.dark
    ? level.soft
    : switch (level) {
        NetLevel.good => c.greenSoft,
        NetLevel.okay => c.amberSoft,
        NetLevel.weak => c.redSoft,
        NetLevel.lost => c.hover,
      };

/// Four bars of rising height, as many lit as [level] allows: the same as on a shared
/// screen's quality. Small ones go in lists and lines of text.
class SignalBars extends StatelessWidget {
  const SignalBars({
    super.key,
    required this.level,
    this.small = false,
    this.color,
    this.unlit,
  });
  final NetLevel level;
  final bool small;

  /// The lit bars' colour; the level's own when left out.
  final Color? color;
  final Color? unlit;

  static const _tall = [4.0, 7.0, 10.0, 13.0];
  static const _short = [4.0, 6.0, 8.5, 11.0];

  @override
  Widget build(BuildContext context) {
    final heights = small ? _short : _tall;
    final lit = color ?? level.color;
    final off =
        unlit ??
        (context.fg.dark
            ? const Color.fromRGBO(255, 255, 255, .22)
            : const Color.fromRGBO(0, 0, 0, .16));
    return SizedBox(
      height: heights.last,
      child: Row(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.end,
        children: [
          for (final (i, h) in heights.indexed) ...[
            if (i > 0) SizedBox(width: small ? 1.5 : 2),
            AnimatedContainer(
              duration: const Duration(milliseconds: 300),
              width: small ? 2.5 : 3,
              height: h,
              decoration: BoxDecoration(
                color: i < level.lit ? lit : off,
                borderRadius: BorderRadius.circular(1),
              ),
            ),
          ],
        ],
      ),
    );
  }
}

/// The call's quality, if this device is in a call.
CallQuality? _quality(BuildContext context) =>
    AppScope.of(context).call?.quality;

/// Someone's name as the call shows it.
String _nameOf(AppState state, String id) {
  for (final p
      in state.roomState.room?.participants ?? const <UiParticipant>[]) {
    if (p.person.id == id) return p.person.name;
  }
  return l10n.appSomeone;
}

UiPerson? _personOf(AppState state, String id) {
  final mine = state.contacts.profile?.person;
  if (mine != null && mine.id == id) return mine;
  for (final p
      in state.roomState.room?.participants ?? const <UiParticipant>[]) {
    if (p.person.id == id) return p.person;
  }
  return null;
}

/// The connection of the person a tile shows, in its top right corner: the bars, with
/// the level's word unless it is good, or "Reconnecting…" once nothing comes. A click
/// opens the details. Nothing shows until the connection is measured, nor while it is good
/// ([WhileNotGood]).
class TileQuality extends StatelessWidget {
  const TileQuality({super.key, required this.id});
  final String id;

  @override
  Widget build(BuildContext context) {
    final quality = _quality(context);
    if (quality == null) return const SizedBox.shrink();
    return ListenableBuilder(
      listenable: quality,
      builder: (context, _) {
        final level = quality.shownOf(id);
        return AnimatedSwitcher(
          duration: const Duration(milliseconds: 200),
          child: level == null
              ? const SizedBox.shrink()
              : WhileNotGood(
                  level: level,
                  child: _Details(
                    id: id,
                    builder: (context, open) => Pressable(
                      onTap: open,
                      focusRadius: 999,
                      label: id == quality.me
                          ? l10n.qualityYourDetails(level.word)
                          : l10n.qualityTheirDetails(
                              _nameOf(AppScope.read(context), id),
                              level.word,
                            ),
                      builder: (context, hovered) => AnimatedScale(
                        scale: hovered ? 1.06 : 1,
                        duration: springTime,
                        curve: spring,
                        child: _Chip(level: level),
                      ),
                    ),
                  ),
                ),
        );
      },
    );
  }
}

/// How long a connection stays good before its bars go.
const goodFor = Duration(seconds: 4);

/// Shows [child] while [level] is not good: from the moment it gets worse, until it has
/// been good again for [goodFor]. A connection good from the start never shows it.
class WhileNotGood extends StatefulWidget {
  const WhileNotGood({super.key, required this.level, required this.child});
  final NetLevel level;
  final Widget child;

  @override
  State<WhileNotGood> createState() => _WhileNotGoodState();
}

class _WhileNotGoodState extends State<WhileNotGood> {
  late bool _shown = widget.level != NetLevel.good;
  Timer? _hide;

  @override
  void didUpdateWidget(WhileNotGood old) {
    super.didUpdateWidget(old);
    if (widget.level != NetLevel.good) {
      _hide?.cancel();
      _hide = null;
      _shown = true;
    } else if (_shown && _hide == null) {
      _hide = Timer(goodFor, () {
        _hide = null;
        if (mounted) setState(() => _shown = false);
      });
    }
  }

  @override
  void dispose() {
    _hide?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => AnimatedSwitcher(
    duration: const Duration(milliseconds: 200),
    child: _shown ? widget.child : const SizedBox.shrink(),
  );
}

/// The pill on a tile: over a picture, so in the same colours in either theme.
class _Chip extends StatelessWidget {
  const _Chip({required this.level});
  final NetLevel level;

  @override
  Widget build(BuildContext context) {
    final lost = level == NetLevel.lost;
    return Container(
      height: 26,
      padding: const EdgeInsets.symmetric(horizontal: 9),
      decoration: BoxDecoration(
        color: const Color.fromRGBO(0, 0, 0, .6),
        borderRadius: BorderRadius.circular(999),
        border: Border.all(color: const Color.fromRGBO(255, 255, 255, .08)),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          if (lost)
            Spinner(size: 12, stroke: 1.5, color: level.color)
          else
            SignalBars(
              level: level,
              unlit: const Color.fromRGBO(255, 255, 255, .22),
            ),
          if (level != NetLevel.good) ...[
            const SizedBox(width: 7),
            Text(
              lost ? l10n.qualityReconnecting : level.word,
              style: tx(level.color, size: 12, weight: w500, height: 1.2),
            ),
          ],
        ],
      ),
    );
  }
}

/// Dims someone's picture while nothing comes from them; never this device's own, which
/// stays here whatever its network does.
class QualityDim extends StatelessWidget {
  const QualityDim({super.key, required this.id, required this.child});

  /// Whose picture; null leaves it as it is.
  final String? id;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    final id = this.id;
    final quality = id == null ? null : _quality(context);
    if (id == null || quality == null || id == quality.me) return child;
    return ListenableBuilder(
      listenable: quality,
      child: child,
      builder: (context, child) => AnimatedOpacity(
        opacity: quality.shownOf(id) == NetLevel.lost ? .35 : 1,
        duration: const Duration(milliseconds: 300),
        child: child,
      ),
    );
  }
}

/// In the list of who is in the room: small bars only when someone's connection is not
/// good, or a spinner while nothing comes from them; nothing otherwise, so the list stays
/// calm. This device's own row only shows the spinner.
class QualityMark extends StatelessWidget {
  const QualityMark({super.key, required this.id});
  final String id;

  @override
  Widget build(BuildContext context) {
    final quality = _quality(context);
    if (quality == null) return const SizedBox.shrink();
    return ListenableBuilder(
      listenable: quality,
      builder: (context, _) {
        final level = quality.shownOf(id);
        final c = context.fg;
        if (level == NetLevel.lost) {
          return Padding(
            padding: const EdgeInsets.only(left: 6),
            child: Semantics(
              label: l10n.qualityReconnectingLabel,
              child: Spinner(size: 12, stroke: 1.5, color: c.fg2),
            ),
          );
        }
        if (level == null || level == NetLevel.good || id == quality.me) {
          return const SizedBox.shrink();
        }
        return Padding(
          padding: const EdgeInsets.only(left: 6),
          child: Semantics(
            label: l10n.qualityLevelConnection(level.name),
            child: SignalBars(
              level: level,
              small: true,
              color: _levelColor(c, level),
            ),
          ),
        );
      },
    );
  }
}

/// The state under the person's name in their own card while they are in a call: the
/// words, or "Poor connection" or "Reconnecting…" when it comes to that, and the bars of
/// their own connection; a click on the bars opens its details above [card]. [fallback]
/// shows until a connection is measured.
class OwnQuality extends StatelessWidget {
  const OwnQuality({
    super.key,
    required this.words,
    required this.color,
    required this.card,
    required this.fallback,
    this.suffix,
  });
  final String words;
  final Color color;

  /// Said after the words while the connection is fine (the other person of a call).
  final String? suffix;
  final BuildContext card;
  final Widget fallback;

  @override
  Widget build(BuildContext context) {
    final quality = _quality(context);
    if (quality == null) return fallback;
    return ListenableBuilder(
      listenable: quality,
      builder: (context, _) {
        final level = quality.mine;
        if (level == null) return fallback;
        final c = context.fg;
        final fine = !level.bad;
        final color = fine ? this.color : _levelColor(c, level);
        final style = tx(color, size: 11, height: 1.3);
        final suffix = this.suffix;
        return LayoutBuilder(
          builder: (context, box) => Row(
            children: [
              if (fine)
                Flexible(
                  flex: 2,
                  child: Text(
                    words,
                    maxLines: 1,
                    softWrap: false,
                    overflow: TextOverflow.ellipsis,
                    style: style,
                  ),
                )
              else
                Flexible(
                  child: Text(
                    level == NetLevel.lost
                        ? l10n.qualityReconnecting
                        // The card is narrow: the word alone when both do not fit.
                        : _fits(
                            context,
                            l10n.qualityPoorConnection,
                            style,
                            box.maxWidth - 24,
                          )
                        ? l10n.qualityPoorConnection
                        : l10n.qualityPoor,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: style,
                  ),
                ),
              if (fine && suffix != null)
                Flexible(
                  child: Text(
                    suffix,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: style,
                  ),
                ),
              WhileNotGood(
                level: level,
                child: Padding(
                  padding: const EdgeInsets.only(left: 3),
                  child: _Details(
                    id: quality.me,
                    box: card,
                    builder: (context, open) => Pressable(
                      onTap: open,
                      focusRadius: 4,
                      label: l10n.qualityYourDetails(level.word),
                      builder: (context, hovered) => Padding(
                        padding: const EdgeInsets.all(3),
                        child: level == NetLevel.lost
                            ? Spinner(size: 11, stroke: 1.5, color: color)
                            : SignalBars(
                                level: level,
                                small: true,
                                color: _levelColor(c, level),
                              ),
                      ),
                    ),
                  ),
                ),
              ),
            ],
          ),
        );
      },
    );
  }

  static bool _fits(
    BuildContext context,
    String text,
    TextStyle style,
    double width,
  ) {
    final painter = TextPainter(
      text: TextSpan(text: text, style: style),
      textDirection: TextDirection.ltr,
      textScaler: MediaQuery.textScalerOf(context),
      maxLines: 1,
    )..layout();
    final fits = painter.width <= width;
    painter.dispose();
    return fits;
  }
}

/// A line above the call's controls while this device's own network is poor, so the
/// person knows the others may hear them choppy. It goes 10 s after the network is fine
/// again.
class OwnNetworkLine extends StatefulWidget {
  const OwnNetworkLine({super.key});

  /// How long the line stays once the network is fine again.
  static const linger = Duration(seconds: 10);

  @override
  State<OwnNetworkLine> createState() => _OwnNetworkLineState();
}

class _OwnNetworkLineState extends State<OwnNetworkLine> {
  CallQuality? _quality;
  bool _shown = false;
  Timer? _hide;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final quality = AppScope.of(context).call?.quality;
    if (quality == _quality) return;
    _quality?.removeListener(_changed);
    _quality = quality?..addListener(_changed);
    _changed();
  }

  @override
  void dispose() {
    _quality?.removeListener(_changed);
    _hide?.cancel();
    super.dispose();
  }

  void _changed() {
    final poor = _quality?.network?.bad ?? false;
    if (poor) {
      _hide?.cancel();
      _hide = null;
      if (!_shown) setState(() => _shown = true);
    } else if (_shown && _hide == null) {
      // Gone at once when the call ends; otherwise a while after the network recovers.
      if (_quality == null) {
        setState(() => _shown = false);
        return;
      }
      _hide = Timer(OwnNetworkLine.linger, () {
        _hide = null;
        if (mounted) setState(() => _shown = false);
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final text = c.dark ? const Color(0xFFFF8A8E) : c.red;
    return AnimatedSize(
      duration: const Duration(milliseconds: 220),
      curve: Curves.easeOutCubic,
      alignment: Alignment.bottomCenter,
      child: !_shown
          ? const SizedBox(width: double.infinity)
          : Padding(
              padding: const EdgeInsets.only(bottom: 10),
              child: Center(
                child: _Pop(
                  alignment: Alignment.bottomCenter,
                  child: Container(
                    height: 30,
                    padding: const EdgeInsets.symmetric(horizontal: 12),
                    decoration: BoxDecoration(
                      color: c.dark
                          ? const Color.fromRGBO(255, 97, 102, .12)
                          : c.redSoft,
                      borderRadius: BorderRadius.circular(999),
                    ),
                    child: Row(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        SignalBars(
                          level: NetLevel.weak,
                          small: true,
                          color: _levelColor(c, NetLevel.weak),
                        ),
                        const SizedBox(width: 8),
                        Flexible(
                          child: Text(
                            l10n.qualityMinePoorBanner,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: tx(
                              text,
                              size: 12,
                              weight: w500,
                              height: 1.2,
                            ),
                          ),
                        ),
                      ],
                    ),
                  ),
                ),
              ),
            ),
    );
  }
}

/// Grows in from a little smaller and above, with the controls' spring.
class _Pop extends StatelessWidget {
  const _Pop({required this.child, required this.alignment});
  final Widget child;
  final Alignment alignment;

  @override
  Widget build(BuildContext context) => TweenAnimationBuilder<double>(
    tween: Tween(begin: 0, end: 1),
    duration: springTime,
    curve: spring,
    child: child,
    builder: (context, t, child) => Opacity(
      opacity: t.clamp(0.0, 1.0),
      child: Transform.translate(
        offset: Offset(0, -6 * (1 - t)),
        child: Transform.scale(
          scale: .95 + .05 * t,
          alignment: alignment,
          child: child,
        ),
      ),
    ),
  );
}

/// Opens the details of [id]'s connection over everything, under the widget that
/// [builder] makes (a tile's pill), or above [box] when given (the own card). A click
/// anywhere else or Escape closes them.
class _Details extends StatefulWidget {
  const _Details({required this.id, required this.builder, this.box});
  final String id;
  final BuildContext? box;
  final Widget Function(BuildContext context, VoidCallback open) builder;

  @override
  State<_Details> createState() => _DetailsState();
}

class _DetailsState extends State<_Details> {
  final _portal = OverlayPortalController();

  void _toggle() {
    if (_portal.isShowing) {
      _close();
      return;
    }
    HardwareKeyboard.instance.addHandler(_key);
    setState(_portal.show);
  }

  void _close() {
    HardwareKeyboard.instance.removeHandler(_key);
    if (_portal.isShowing && mounted) setState(_portal.hide);
  }

  bool _key(KeyEvent event) {
    if (event is KeyDownEvent &&
        event.logicalKey == LogicalKeyboardKey.escape) {
      _close();
      return true;
    }
    return false;
  }

  @override
  void dispose() {
    HardwareKeyboard.instance.removeHandler(_key);
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => OverlayPortal.overlayChildLayoutBuilder(
    controller: _portal,
    overlayChildBuilder: (context, info) {
      var anchor = MatrixUtils.transformRect(
        info.childPaintTransform,
        Offset.zero & info.childSize,
      );
      final above = widget.box != null;
      final box = widget.box?.findRenderObject();
      if (box is RenderBox && box.attached && box.hasSize) {
        // The window's overlay starts at its top left corner.
        anchor = box.localToGlobal(Offset.zero) & box.size;
      }
      return Stack(
        children: [
          Positioned.fill(
            child: GestureDetector(
              behavior: HitTestBehavior.opaque,
              onTap: _close,
            ),
          ),
          Positioned.fill(
            child: CustomSingleChildLayout(
              delegate: _Place(anchor: anchor, above: above),
              child: _Pop(
                alignment: above
                    ? Alignment.bottomLeft
                    : const Alignment(.8, -1),
                child: ConnectionDetails(id: widget.id),
              ),
            ),
          ),
        ],
      );
    },
    child: widget.builder(context, _toggle),
  );
}

/// Puts the details under the anchor, their right edges lined up, or above it when the
/// window has no room below; above the own card, their left edges lined up. Always 8 px
/// inside the window.
class _Place extends SingleChildLayoutDelegate {
  _Place({required this.anchor, required this.above});
  final Rect anchor;
  final bool above;

  static const _gap = 8.0;

  @override
  BoxConstraints getConstraintsForChild(BoxConstraints constraints) =>
      constraints.loosen();

  @override
  Offset getPositionForChild(Size size, Size child) {
    var x = above ? anchor.left : anchor.right - child.width;
    var y = above ? anchor.top - _gap - child.height : anchor.bottom + _gap;
    if (!above && y + child.height > size.height - _gap) {
      y = anchor.top - _gap - child.height;
    }
    x = x.clamp(_gap, math.max(_gap, size.width - child.width - _gap));
    y = y.clamp(_gap, math.max(_gap, size.height - child.height - _gap));
    return Offset(x, y);
  }

  @override
  bool shouldRelayout(_Place old) => old.anchor != anchor || old.above != above;
}

/// The details of [id]'s connection: its level, the connection between this device and
/// them and their own network, the delay, the packets lost and the way it goes, and a
/// sentence on whose side any trouble is. For this device: its own network and its
/// connection to each of the others.
class ConnectionDetails extends StatelessWidget {
  const ConnectionDetails({super.key, required this.id});
  final String id;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final quality = state.callState.call?.quality;
    if (quality == null) return const SizedBox.shrink();
    return ListenableBuilder(
      listenable: Listenable.merge([quality, quality.readings]),
      builder: (context, _) => _build(context, state, quality),
    );
  }

  Widget _build(BuildContext context, AppState state, CallQuality quality) {
    final c = context.fg;
    final shown = quality.shownOf(id);
    if (shown == null) return const SizedBox.shrink();
    final now = DateTime.now();
    final you = id == quality.me;
    final person = _personOf(state, id);
    final name = person?.name ?? _nameOf(state, id);
    final paths =
        state.contacts.paths[state.callState.call?.group] ??
        const <String, UiPath>{};
    final rows = <Widget>[];
    final List<(String, String)> figures;
    final String hint;
    if (you) {
      final network = quality.network;
      if (network != null) {
        rows.add(_LevelRow(label: l10n.qualityYourNetwork, level: network));
      }
      final others = <OtherLink>[];
      for (final other in quality.measured) {
        final link = quality.linkOf(other)!;
        final otherName = _nameOf(state, other);
        final delay = quality.delayOf(other);
        rows.add(
          _LevelRow(
            label: l10n.qualityTo(otherName),
            level: link,
            delay: link == NetLevel.lost || delay == null
                ? null
                : delayText(delay),
          ),
        );
        others.add((
          name: otherName,
          link: link,
          theirs: quality.theirsOf(other),
          throughMember: paths[other] == UiPath.throughMember,
          through: state.contacts.carrierName(
            state.callState.call?.group,
            other,
          ),
          silence: quality.silenceOf(other, now) ?? Duration.zero,
        ));
      }
      figures = const [];
      hint = hintAboutMe(network: network, others: others);
    } else {
      final link = quality.linkOf(id)!;
      final theirs = quality.theirsOf(id);
      rows.add(_LevelRow(label: l10n.qualityBetween(name), level: link));
      if (theirs != null) {
        rows.add(
          _LevelRow(label: l10n.qualityTheirNetwork(name), level: theirs),
        );
      }
      final silence = quality.silenceOf(id, now) ?? Duration.zero;
      final path = paths[id];
      final through = state.contacts.carrierName(
        state.callState.call?.group,
        id,
      );
      if (link == NetLevel.lost) {
        figures = [
          (l10n.qualityLastHeard, l10n.qualitySecondsAgo(silence.inSeconds)),
          (l10n.qualityPath, l10n.qualityLookingAgain),
        ];
      } else {
        final delay = quality.delayOf(id);
        final loss = quality.lossOf(id);
        figures = [
          (l10n.qualityDelayLabel, delay == null ? '—' : delayText(delay)),
          (l10n.qualityLostPackets, loss == null ? '—' : lossText(loss)),
          if (path != null)
            (l10n.qualityPath, capitalized(pathWords(path, through: through))),
        ];
      }
      hint = hintAbout(
        name: name,
        link: link,
        theirs: theirs,
        mine: quality.network,
        throughMember: path == UiPath.throughMember,
        through: through,
        silence: silence,
      );
    }
    final levelColor = _levelColor(c, shown);
    return Container(
      width: 320,
      padding: const EdgeInsets.all(14),
      decoration: BoxDecoration(
        color: c.bg2,
        borderRadius: BorderRadius.circular(18),
        border: Border.all(color: c.dark ? c.raised : c.border),
      ),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            children: [
              if (person != null) ...[
                Avatar(avatar: person.avatar, size: 30),
                const SizedBox(width: 10),
              ],
              Expanded(
                child: Text(
                  you
                      ? l10n.qualityYourConnection
                      : l10n.qualityTheirConnection(name),
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: tx(c.fg, weight: w600),
                ),
              ),
              const SizedBox(width: 10),
              Container(
                height: 24,
                padding: const EdgeInsets.symmetric(horizontal: 9),
                alignment: Alignment.center,
                decoration: BoxDecoration(
                  color: _levelSoft(c, shown),
                  borderRadius: BorderRadius.circular(999),
                ),
                child: Text(
                  shown.word,
                  style: tx(levelColor, size: 12, weight: w500, height: 1.2),
                ),
              ),
            ],
          ),
          const SizedBox(height: 12),
          Container(
            padding: const EdgeInsets.all(4),
            decoration: BoxDecoration(
              color: c.bg1,
              borderRadius: BorderRadius.circular(12),
              border: Border.all(
                color: c.dark ? const Color(0xFF1C1C1C) : c.border,
              ),
            ),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                for (final (i, row) in rows.indexed) ...[
                  if (i > 0) const SizedBox(height: 2),
                  row,
                ],
              ],
            ),
          ),
          if (figures.isNotEmpty) ...[
            const SizedBox(height: 12),
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 4),
              child: Column(
                mainAxisSize: MainAxisSize.min,
                children: [
                  for (final (i, (label, value)) in figures.indexed) ...[
                    if (i > 0) const SizedBox(height: 4),
                    Row(
                      children: [
                        Text(label, style: tx(c.fg2, size: 13)),
                        const SizedBox(width: 12),
                        Expanded(
                          child: Text(
                            value,
                            textAlign: TextAlign.right,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: tx(
                              c.fg,
                              size: 13,
                              monospace: label != l10n.qualityPath,
                            ),
                          ),
                        ),
                      ],
                    ),
                  ],
                ],
              ),
            ),
          ],
          const SizedBox(height: 12),
          Container(
            padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
            decoration: BoxDecoration(
              color: c.dark ? const Color(0xFF161616) : c.hover,
              borderRadius: BorderRadius.circular(12),
            ),
            child: Text(
              hint,
              style: tx(
                c.dark ? const Color(0xFFBDBDBD) : c.fg2,
                size: 12,
                height: 1.45,
              ),
            ),
          ),
        ],
      ),
    );
  }
}

/// A level in the details: what it is of, the delay to someone for this device's own,
/// the bars and the word.
class _LevelRow extends StatelessWidget {
  const _LevelRow({required this.label, required this.level, this.delay});
  final String label;
  final NetLevel level;
  final String? delay;

  @override
  Widget build(BuildContext context) {
    final c = context.fg;
    final color = _levelColor(c, level);
    final delay = this.delay;
    return ConstrainedBox(
      constraints: const BoxConstraints(minHeight: 32),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 8),
        child: Row(
          children: [
            Expanded(
              child: Text(
                label,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: tx(c.fg2, size: 13),
              ),
            ),
            if (delay != null) ...[
              const SizedBox(width: 10),
              Text(delay, style: tx(c.fg3, size: 11, monospace: true)),
            ],
            const SizedBox(width: 10),
            SignalBars(level: level, small: true, color: color),
            const SizedBox(width: 10),
            ConstrainedBox(
              constraints: const BoxConstraints(minWidth: 44),
              child: Text(
                level.word,
                textAlign: TextAlign.right,
                style: tx(color, size: 13, weight: w500),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
