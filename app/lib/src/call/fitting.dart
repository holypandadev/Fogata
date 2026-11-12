part of '../call.dart';

/// How a shared screen goes out: at most [lines] lines high (720 or 1080, in a 16:9 box),
/// or as large as it is captured when null, and at most [fps] pictures a second.
@immutable
class ScreenQuality {
  const ScreenQuality({this.lines = 1080, this.fps = 30});

  /// The choices offered, in order.
  static const resolutions = <int?>[720, 1080, null];
  static const rates = [15, 30, 60];

  final int? lines;
  final int fps;

  String get resolutionLabel => lines == null ? l10n.callSource : '${lines}p';
  String get label => l10n.callQuality(resolutionLabel, fps);

  Map<String, Object?> toJson() => {'lines': lines, 'fps': fps};

  /// Read back from the settings; anything else than the choices offered is the default.
  static ScreenQuality fromJson(Object? json) {
    if (json is! Map) return const ScreenQuality();
    final lines = json['lines'];
    final fps = json['fps'];
    return ScreenQuality(
      lines: resolutions.contains(lines) ? lines as int? : 1080,
      fps: rates.contains(fps) ? fps as int : 30,
    );
  }

  @override
  bool operator ==(Object other) =>
      other is ScreenQuality && other.lines == lines && other.fps == fps;

  @override
  int get hashCode => Object.hash(lines, fps);
}

/// How much a shared screen captured at `width` by `height` is scaled down to fit in the
/// 16:9 box of `lines` lines (1920 by 1080 for 1080), keeping its shape; 1 for a screen
/// that fits already. When `lines` is null (as captured), only a screen larger than any
/// participant takes ([fairPicture]) is scaled down, to fit.
@visibleForTesting
double screenScale(double width, double height, {int? lines = 1080}) {
  if (lines == null) {
    if (width <= maxScreenSide &&
        height <= maxScreenSide &&
        width * height <= maxScreenPixels) {
      return 1;
    }
    // WebRTC rounds each side of a picture scaled down to the nearest pixel, which can
    // add up to half a pixel on each: a few thousand pixels below the limit, the picture
    // still fits once rounded.
    return _fitWithin(
      width,
      height,
      maxScreenSide,
      maxScreenPixels - 2 * maxScreenSide,
    );
  }
  return math.max(1.0, math.max(width / (lines * 16 / 9), height / lines));
}

/// How much a picture of `width` by `height` is scaled down so that neither side is over
/// `side` and it has at most `pixels`; 1 when it fits.
double _fitWithin(double width, double height, int side, int pixels) =>
    math.max(
      1.0,
      math.max(
        math.max(width, height) / side,
        math.sqrt(width * height / pixels),
      ),
    );

/// Largest camera picture taken from a participant: Full HD, either way up. No copy sends
/// a larger one (see [cameraScale]).
const maxCameraSide = 1920;
const maxCameraPixels = 1920 * 1080;

/// Largest shared screen taken from a participant: 8K, no side over 8192, as the
/// renderer draws no larger and the VP8 decoder is given no larger (see FOGATA.md and
/// tools/webrtc/patches/wrapper/0003). No copy sends a larger one (see [screenScale]).
const maxScreenSide = 8192;
const maxScreenPixels = 7680 * 4320;

/// The sizes, in lines, a shared screen steps down to when the connection cannot carry it
/// smoothly at its chosen size; text stays readable down to the last.
const screenSteps = [1080, 720, 540];

/// The most lines a shared screen goes out at with [watchers] watching it, as the 16:9 box
/// of [ScreenQuality.lines]; null for no more than the quality chosen. Everyone watching
/// has an encoder of their own for the same picture, and each one's memory and work grow
/// with its pixels: shared to seven, a 1600×900 screen took the sharer about 1 GB and a
/// core. One or two watchers get the quality chosen; three or four at most 1080 lines,
/// where a screen shared at its own size (1440 lines, or 4K) would take two to four times
/// that; five or more at most 720, the smaller quality offered, so that what [ScreenLoad]
/// gives back when the processor allows is frames rather than size.
int? screenCeiling(int watchers) => switch (watchers) {
  <= 2 => null,
  <= 4 => 1080,
  _ => 720,
};

/// The lines of the 16:9 box a shared screen goes out in: those of the quality [chosen]
/// (null: as captured), at most [ceiling] (null: none). The ceiling never makes it larger
/// than chosen, and [screenScale] never larger than captured.
int? cappedLines(int? chosen, int? ceiling) => chosen == null
    ? ceiling
    : ceiling == null
    ? chosen
    : math.min(chosen, ceiling);

/// How this device's shared screen reaches one person watching it: the size and rate it
/// goes out at, whether their connection made it smaller than the chosen quality
/// ([lowered]), and whether it goes out smaller or slower than chosen to everyone, to spare
/// this device ([spared]: for the number watching, see [screenCeiling], or for its
/// processor, see [ScreenLoad]).
typedef ScreenOut = ({
  int width,
  int height,
  double fps,
  bool lowered,
  bool spared,
});

/// How a shared screen goes out to one person. It keeps its size, so text stays sharp, and
/// WebRTC drops frames when the connection falls short. When that leaves it choppy (see
/// [choppyUnder], for 6 s), it goes out a step smaller (see [screenSteps]); once a smaller
/// size goes out at nearly the full rate for a while, it tries the size above again,
/// waiting twice as long each time that did not last. The first 10 s are not judged:
/// WebRTC starts slow and finds the connection's rate then.
@visibleForTesting
class ScreenFit {
  /// Under how many frames a second a screen meant to go out at [wanted] is choppy: under
  /// 10, or half its rate, for a screen of 30 frames a second or fewer, where sharp text
  /// matters more than motion; under three quarters of it above that, as a rate of 60 is
  /// chosen for motion: kept at its size at 25 or 30 of its 60 frames a second, as a
  /// connection that falls short left it, it moved no better than a screen chosen at 30.
  static double choppyUnder(double wanted) =>
      wanted > 30 ? wanted * 0.75 : math.min(10, wanted / 2);

  /// How many steps below its chosen size the screen goes out now.
  int step = 0;

  /// How long a smaller size must go out at nearly the full rate before the size above is
  /// tried.
  Duration wait = _firstWait;

  static const _firstWait = Duration(seconds: 10);
  static const _longestWait = Duration(seconds: 160);
  static const _settling = Duration(seconds: 10);

  /// The first reading since the screen started going out, or since it started over.
  DateTime? _first;

  /// Readings in a row that were choppy.
  int _choppy = 0;

  /// Since when the screen goes out at nearly the full rate.
  DateTime? _smooth;

  /// When the size above was last tried.
  DateTime? _tried;

  /// The sizes the screen goes out at, in lines, from its own height at its chosen
  /// quality: that height, then each of [screenSteps] below it.
  static List<int> sizes(int lines) => [
    lines,
    for (final step in screenSteps)
      if (step < lines) step,
  ];

  /// Takes a reading of what goes out: [fps] frames a second, of [wanted] (what the screen
  /// is captured at, at most its quality's rate), with [steps] sizes to go through. Returns
  /// whether [step] changed.
  bool take({
    required double fps,
    required double wanted,
    required int steps,
    required DateTime now,
  }) {
    if (wanted <= 0) return false;
    final first = _first ??= now;
    if (now.difference(first) < _settling) return false;
    if (fps < choppyUnder(wanted)) {
      _smooth = null;
      if (++_choppy < 3 || step >= steps - 1) return false;
      _choppy = 0;
      step++;
      final tried = _tried;
      if (tried != null &&
          now.difference(tried) < const Duration(seconds: 20)) {
        wait = wait * 2 > _longestWait ? _longestWait : wait * 2;
      }
      return true;
    }
    _choppy = 0;
    if (step == 0 || fps < wanted * 0.8) {
      _smooth = null;
      return false;
    }
    final since = _smooth ??= now;
    if (now.difference(since) < wait) return false;
    step--;
    _tried = now;
    _smooth = null;
    return true;
  }

  /// Starts over, as for a new share or another quality.
  void reset() {
    step = 0;
    _first = null;
    wait = _firstWait;
    _choppy = 0;
    _smooth = null;
    _tried = null;
  }
}

/// How much of this device's processor a shared screen may take. Everyone watching has an
/// encoder of their own for the same picture, so the work grows with each one: on a
/// 16-thread desktop, five watchers of a 1600×900 screen took 2.7 cores and seven brought it
/// down to 5 frames a second for everyone, as measured. It also keeps the screen within
/// the [ceiling] the number watching calls for (see [screenCeiling]). Unlike [ScreenFit],
/// which follows one person's connection, this follows the encoders together and lowers
/// the screen for everyone at once, a level at a time (see [levels]): when WebRTC says an
/// encoder is held back by the processor (`qualityLimitationReason` "cpu") or the encoders
/// together keep more than [busyCores] cores busy, for two readings in a row (4 s). Once
/// nothing is held back for [wait], and the level above would keep them under 90 % of
/// [busyCores] (what they take now, grown by the level's pixels and frames), it goes a
/// level back up,
/// and waits twice as long (up to 160 s) each time that did not last 20 s. A share starts
/// lower with more watchers ([startFor]), and goes lower when more come. The rate halves a
/// level at a time (60, 30, 15) before the size goes down: one moment of load once took a
/// 60 frames a second screen straight to 15, and the level above then had to fit in a
/// quarter of the cores it took, so it seldom came back. A level that sends the same as
/// the one above it, as 60 and 30 do for a screen captured at 30, is passed over.
@visibleForTesting
class ScreenLoad {
  /// The most lines and frames a second the screen goes out at, by level; null lines
  /// keeps the quality's own size.
  static const levels = <({int? lines, int fps})>[
    (lines: null, fps: 60),
    (lines: null, fps: 30),
    (lines: null, fps: 15),
    (lines: 720, fps: 15),
    (lines: 540, fps: 15),
    (lines: 540, fps: 10),
    (lines: 540, fps: 5),
  ];

  /// Processor cores the screen's encoders may keep busy together before the screen goes
  /// a level down.
  static const busyCores = 1.5;

  /// After a change of level, how long WebRTC's own word that the processor holds an
  /// encoder back is not taken: its own lowering (fewer frames, with the size kept) undoes
  /// itself a step at a time, 10 s or more each, and says "cpu" meanwhile.
  static const settling = Duration(seconds: 12);

  static const _firstWait = Duration(seconds: 20);
  static const _longestWait = Duration(seconds: 160);

  /// The level a share starts at with [watchers] watching: the full screen for one or
  /// two, 15 frames a second for three, 720 lines at 15 for four, and 540 lines at 15
  /// from five, where it would otherwise begin larger and fall: in the lab, three
  /// watchers of a 1600×900 screen kept 15 frames a second at its full size, but five
  /// started at 720 lines went down to 10. 540 lines at 15 takes seven encoders about
  /// what 720 lines took five. With fewer pixels from five on (see [screenCeiling]), a
  /// level up there brings back frames rather than size.
  static int startFor(int watchers) => switch (watchers) {
    <= 2 => 0,
    3 => 2,
    4 => 3,
    _ => 4,
  };

  /// How much encoding a level takes, as lines squared times frames a second, with the
  /// screen captured at most [fps] frames a second and kept within [ceiling] lines: the
  /// full screen counts as 1080 lines.
  @visibleForTesting
  static double costOf(int level, {int fps = 30, int? ceiling}) {
    final limits = levels[level];
    final lines = (cappedLines(limits.lines, ceiling) ?? 1080).toDouble();
    return lines * lines * math.min(limits.fps, fps);
  }

  /// The most lines the screen goes out at for the number watching (see
  /// [screenCeiling]), whatever the level; null for none. It comes down at once when
  /// more watch, before the new watcher's encoder starts, and goes back up only once
  /// fewer have watched for [ceilingWait]: someone who comes and goes within it leaves
  /// it where it is.
  int? ceiling;

  /// How long fewer must watch before the [ceiling] goes back up.
  static const ceilingWait = Duration(seconds: 20);

  /// Since when fewer watch than the [ceiling] is for, and the most that watched since.
  DateTime? _fewerSince;
  int _mostSinceFewer = 0;

  /// The most frames a second the screen is captured at (its quality's, or less as it
  /// was asked for); the call sets it.
  int capturedFps = 30;

  /// Which of [levels] the screen goes out at now.
  int level = 0;

  /// How long every reading must be light before a level up is tried.
  Duration wait = _firstWait;

  int _heavy = 0;
  DateTime? _light;
  DateTime? _tried;

  /// When the level last changed.
  DateTime? _changed;

  /// Each encoder's frames a second at the last reading.
  Map<String, double> _fpsOut = {};

  /// Since when the encoders together keep under a quarter of [busyCores]; after
  /// [_idleAfter] of that, WebRTC's "cpu" no longer holds a level up back.
  DateTime? _idle;
  static const _idleAfter = Duration(seconds: 20);

  /// The most watchers since the level last changed, and at the last reading.
  int _mostWatchers = 0;
  int _watchers = 0;

  /// Each encoder's time spent encoding (seconds) at the last reading, and when.
  Map<String, double> _encoded = {};
  DateTime? _at;

  ({int? lines, int fps}) get limits => levels[level];

  /// Whether the screen goes out smaller or at fewer frames than at the first level.
  bool get lowered => _sent(level) != _sent(0);

  /// The most frames a second the screen goes out at now.
  int get _fps => math.min(limits.fps, capturedFps);

  /// What level [at] sends of a screen captured at [capturedFps], within the [ceiling].
  ({int? lines, int fps}) _sent(int at) => (
    lines: cappedLines(levels[at].lines, ceiling),
    fps: math.min(levels[at].fps, capturedFps),
  );

  /// The next level down that sends less than [level]; [level] itself at the last.
  int _below(int level) {
    for (var i = level + 1; i < levels.length; i++) {
      if (_sent(i) != _sent(level)) return i;
    }
    return level;
  }

  /// The first of the levels that send what the one above [level] sends; 0 at the top.
  int _above(int level) {
    var i = level;
    while (i > 0 && _sent(i) == _sent(level)) {
      i--;
    }
    while (i > 0 && _sent(i - 1) == _sent(i)) {
      i--;
    }
    return i;
  }

  void _setLevel(int to, DateTime now) {
    level = to;
    _mostWatchers = _watchers;
    _settle(now);
  }

  /// What the encoders are asked for changed: readings start over, and WebRTC's word that
  /// the processor holds one back is not taken for [settling].
  void _settle(DateTime now) {
    _changed = now;
    _heavy = 0;
    _light = null;
  }

  /// [watchers] watch now: the [ceiling] their number calls for (lower at once, higher
  /// after [ceilingWait]), and, when more than ever since the level last changed, a level
  /// as low as their number calls for, if not lower already. Returns whether [level] or
  /// [ceiling] changed.
  bool watchersAre(int watchers, {DateTime? now}) {
    final at = now ?? DateTime.now();
    final capped = _followCeiling(watchers, at);
    _watchers = watchers;
    final more = watchers > _mostWatchers;
    _mostWatchers = math.max(_mostWatchers, watchers);
    final start = startFor(watchers);
    if (!more || start <= level) return capped;
    _setLevel(start, at);
    return true;
  }

  /// Moves the [ceiling] towards what [watchers] call for, as [ceiling] says; returns
  /// whether it moved.
  bool _followCeiling(int watchers, DateTime now) {
    final wanted = screenCeiling(watchers);
    final current = ceiling;
    final lower = wanted != null && (current == null || wanted < current);
    if (lower || wanted == current) {
      _fewerSince = null;
      if (!lower) return false;
      ceiling = wanted;
      _settle(now);
      return true;
    }
    final since = _fewerSince;
    if (since == null) {
      _fewerSince = now;
      _mostSinceFewer = watchers;
      return false;
    }
    _mostSinceFewer = math.max(_mostSinceFewer, watchers);
    if (now.difference(since) < ceilingWait) return false;
    // As high as the most that watched meanwhile allow; as many as now watch have
    // watched since, for higher still.
    ceiling = screenCeiling(_mostSinceFewer);
    _fewerSince = ceiling == wanted ? null : now;
    _mostSinceFewer = watchers;
    _settle(now);
    return true;
  }

  /// Takes a reading of every encoder of the screen: by watcher, whether WebRTC says the
  /// processor holds it back ([cpu]), its total time spent encoding so far in seconds
  /// ([encodeSeconds]), the frames a second it sends ([fps]) and those the screen is
  /// captured at ([captured]), each null when not told. One held back counts only while it
  /// also sends well under what is captured and under the level's rate, without rising
  /// (WebRTC keeps saying "cpu" while its own lowering lasts, even once the level asks for
  /// less than that; a still screen under Wayland is captured at a few frames a second),
  /// and not for [settling] after a change. Returns whether [level] changed.
  bool take({
    required Map<
      String,
      ({bool cpu, double encodeSeconds, double? fps, double? captured})
    >
    encoders,
    required DateTime now,
  }) {
    final at = _at;
    _at = now;
    final before = _encoded;
    _encoded = {
      for (final MapEntry(:key, :value) in encoders.entries)
        key: value.encodeSeconds,
    };
    if (encoders.isEmpty) return false;
    final changed = _changed;
    final settled = changed == null || now.difference(changed) >= settling;
    final lastFps = _fpsOut;
    _fpsOut = {
      for (final MapEntry(:key, :value) in encoders.entries)
        if (value.fps != null) key: value.fps!,
    };
    // Held back only when WebRTC says so and the frames it sends fall short of those
    // captured (a still screen under Wayland captures few: no shortfall), and of the
    // level's rate, and do not rise again. Rates not told say nothing.
    final held =
        settled &&
        encoders.entries.any((entry) {
          final e = entry.value;
          final fps = e.fps, captured = e.captured, last = lastFps[entry.key];
          return e.cpu &&
              fps != null &&
              captured != null &&
              fps < captured * 0.8 &&
              fps < _fps * 0.8 &&
              (last == null || fps <= last);
        });
    // Cores busy encoding since the last reading; unknown on the first.
    var busy = 0.0;
    if (at != null && now.isAfter(at)) {
      final seconds = now.difference(at).inMilliseconds / 1000;
      for (final MapEntry(:key, :value) in encoders.entries) {
        final last = before[key];
        if (last != null && value.encodeSeconds >= last) {
          busy += (value.encodeSeconds - last) / seconds;
        }
      }
    }
    if (busy < busyCores / 4) {
      _idle ??= now;
    } else {
      _idle = null;
    }
    if (held || busy > busyCores) {
      _light = null;
      final below = _below(level);
      if (++_heavy < 2 || below == level) return false;
      final tried = _tried;
      if (tried != null &&
          now.difference(tried) < const Duration(seconds: 20)) {
        wait = wait * 2 > _longestWait ? _longestWait : wait * 2;
      }
      _setLevel(below, now);
      return true;
    }
    _heavy = 0;
    // No level up while WebRTC still says "cpu", unless the encoders have kept nearly
    // idle for [_idleAfter] (its word can stay stuck with nothing encoded); and one must
    // stay under the limit: what the encoders take now, grown as the level above sends
    // more.
    final idle = _idle;
    final stuck = idle != null && now.difference(idle) >= _idleAfter;
    final above = _above(level);
    if (level == 0 ||
        (encoders.values.any((e) => e.cpu) && !stuck) ||
        busy *
                costOf(above, fps: capturedFps, ceiling: ceiling) /
                costOf(level, fps: capturedFps, ceiling: ceiling) >
            busyCores * 0.9) {
      _light = null;
      return false;
    }
    final since = _light ??= now;
    if (now.difference(since) < wait) return false;
    _setLevel(above, now);
    _tried = now;
    return true;
  }

  /// Starts over, as for a new share.
  void reset() {
    level = 0;
    ceiling = null;
    _fewerSince = null;
    _mostSinceFewer = 0;
    wait = _firstWait;
    _watchers = 0;
    _mostWatchers = 0;
    _changed = null;
    _fpsOut = {};
    _idle = null;
    _heavy = 0;
    _light = null;
    _tried = null;
    _encoded = {};
    _at = null;
  }
}

/// The most a shared screen may use, in kbit/s, whatever its size and rate: what each
/// side's description lets the other send in the screen channel. WebRTC takes the largest
/// codec limit of a description as the most its bandwidth estimate may reach for the whole
/// connection too, so this is also the ceiling of everything sent together.
const screenMaxKbps = 20000;

/// The most a shared screen may use whatever it sends, in kbit/s: the one limit every
/// screen had before the limit grew with the size and rate (see [screenKbps]). WebRTC's
/// own limit for a large picture is 2500, which kept a sharp screen well under 30 frames a
/// second.
const screenLeastKbps = 6000;

/// How many bits a shared screen may spend on each pixel of each frame that goes out, on
/// top of [screenLeastKbps]: about what a moving 1080p screen at 60 frames a second needs
/// to keep its text sharp (12 Mbit/s).
const screenBitsPerPixel = 0.1;

/// The most a shared screen going out at [width] by [height] and [fps] frames a second may
/// use, in kbit/s: [screenBitsPerPixel] of every pixel sent, at least [screenLeastKbps] and
/// at most [screenMaxKbps]. A larger or faster screen needs more for the same sharpness:
/// with one limit for all, 6000 left a 1080p screen at 60 frames a second dropping frames
/// as soon as it moved. The sender sets it on each connection, where it takes the place of
/// the limit in the other side's description (an older version's 6000 still bounds the
/// connection's bandwidth estimate, see [screenMaxKbps]).
@visibleForTesting
int screenKbps(double width, double height, num fps) =>
    (width * height * fps * screenBitsPerPixel / 1000).round().clamp(
      screenLeastKbps,
      screenMaxKbps,
    );

/// The most the camera may use, in kbit/s. Unlimited, WebRTC let it take about 2500 for each
/// member, so four members could try to send three times that at once.
const cameraMaxKbps = 1500;

/// How the camera goes out with some number of people in the call, this device included.
/// Everyone sends their camera to everyone else, with an encoder for each, so a larger
/// call sends each a smaller picture at a lower bitrate, keeping the whole upload under
/// about 3 Mbit/s and the encoders' work about half of seven 720p pictures': [lines] is
/// the most the picture keeps of its height (null: as the camera gives it), [kbps] the
/// most each person gets (the [cameraMaxKbps] each one's description allows is the most
/// anyway) and [fps] the most frames a second (60 is WebRTC's own limit: the camera's 30
/// decides). With two people the camera goes out as it always did.
typedef CameraStep = ({int? lines, int kbps, int fps});

@visibleForTesting
CameraStep cameraStep(int people) => switch (people) {
  <= 2 => (lines: null, kbps: cameraMaxKbps, fps: 60),
  <= 4 => (lines: 540, kbps: 900, fps: 30),
  <= 6 => (lines: 360, kbps: 500, fps: 30),
  _ => (lines: 360, kbps: 350, fps: 20),
};

/// How much a camera that gives pictures [height] lines high (and [width] wide, when
/// known) is scaled down for [step]; 1 for one no higher. Before its first picture, it is
/// taken to give the 720 lines asked. With all its lines, a camera larger than Full HD
/// still goes out at most that large ([fairPicture]).
@visibleForTesting
double cameraScale(double height, CameraStep step, {double width = 0}) {
  final lines = step.lines;
  if (lines == null) {
    return _fitWithin(width, height, maxCameraSide, maxCameraPixels);
  }
  return math.max(1.0, (height > 0 ? height : 720) / lines);
}

/// How much smaller than its step (see [cameraStep]) the camera goes out to one person,
/// from what their connection can carry. Every connection has an encoder of its own fed
/// by the one camera, and none of them makes the camera's own pictures smaller (see
/// [Call._tune]): this makes the picture smaller for a connection that falls short, and
/// only for that one. A connection that can send at least [needs] of the step's bitrate
/// gets the picture at the step's size, one that can send less a picture [factors]
/// times smaller. Two readings in a row that want a smaller picture make it smaller at
/// once, as the larger of the two wants; once every reading for [wait] wanted a larger
/// picture, it goes out as large as the least of them wants, and after twice as long
/// each time a larger picture did not last 20 s (up to 80 s). The first 6 s after the
/// camera starts going out are not judged: the camera's share of the connection grows to
/// its rate then.
@visibleForTesting
class CameraFit {
  /// How many times smaller than the step's size the picture goes out, by [step].
  static const factors = [1.0, 1.5, 2.0, 3.0];

  /// The share of the step's bitrate the connection must carry for each of [factors]
  /// but the last, which goes out whatever it carries.
  static const needs = [0.75, 0.4, 0.22];

  static const _firstWait = Duration(seconds: 10);
  static const _longestWait = Duration(seconds: 80);
  static const _settling = Duration(seconds: 6);

  /// Which of [factors] the picture goes out at now.
  int step = 0;

  /// How long every reading must want a larger picture before it is tried.
  Duration wait = _firstWait;

  double get factor => factors[step];

  DateTime? _first;

  /// Readings in a row that wanted a smaller picture, and the largest of the pictures
  /// they wanted.
  int _low = 0;
  int _lowStep = 0;

  /// Since when every reading wanted a larger picture, and the smallest of the pictures
  /// they wanted.
  DateTime? _good;
  int _goodStep = 0;

  /// When a larger picture was last tried.
  DateTime? _tried;

  /// The step a connection that carries [kbps] gets, with a step bitrate of [stepKbps].
  static int stepFor(double kbps, int stepKbps) {
    for (var i = 0; i < needs.length; i++) {
      if (kbps >= needs[i] * stepKbps) return i;
    }
    return needs.length;
  }

  /// Takes a reading: the connection can send [kbps] for the camera, of the [stepKbps]
  /// the step allows. Returns whether [step] changed.
  bool take({
    required double kbps,
    required int stepKbps,
    required DateTime now,
  }) {
    if (kbps <= 0 || stepKbps <= 0) return false;
    final first = _first ??= now;
    if (now.difference(first) < _settling) return false;
    final wanted = stepFor(kbps, stepKbps);
    if (wanted > step) {
      _good = null;
      _lowStep = _low == 0 ? wanted : math.min(_lowStep, wanted);
      if (++_low < 2) return false;
      _low = 0;
      step = _lowStep;
      final tried = _tried;
      if (tried != null &&
          now.difference(tried) < const Duration(seconds: 20)) {
        wait = wait * 2 > _longestWait ? _longestWait : wait * 2;
      }
      return true;
    }
    _low = 0;
    if (wanted == step) {
      _good = null;
      return false;
    }
    _goodStep = _good == null ? wanted : math.max(_goodStep, wanted);
    final since = _good ??= now;
    if (now.difference(since) < wait) return false;
    step = _goodStep;
    _tried = now;
    _good = null;
    return true;
  }

  /// Starts over at the step's size, as for a camera turned on again or a new
  /// connection.
  void reset() {
    step = 0;
    wait = _firstWait;
    _first = null;
    _low = 0;
    _good = null;
    _tried = null;
  }
}

/// How someone's camera or screen reaches this device, from its connection's statistics.
class StreamQuality {
  const StreamQuality({
    required this.level,
    this.delayMs,
    this.lossPercent,
    this.width,
    this.height,
    this.fps,
  });

  /// 2 good, 1 so-so, 0 poor.
  final int level;

  /// Round trip to them and back, in milliseconds.
  final int? delayMs;

  /// Packets of this picture lost since the previous reading, in percent.
  final double? lossPercent;
  final int? width;
  final int? height;
  final double? fps;

  /// The level from the delay and the losses: past 150 ms or 2 % it is so-so, past
  /// 400 ms or 8 % poor.
  static int levelOf(int? delayMs, double? lossPercent) {
    final delay = delayMs ?? 0;
    final loss = lossPercent ?? 0;
    if (delay > 400 || loss > 8) return 0;
    if (delay > 150 || loss > 2) return 1;
    return 2;
  }
}

/// How much longer than WebRTC aims for someone's voice may wait here after a stall of
/// the network before what waits is dropped (see [Call._flushLateVoice]).
const lateVoice = Duration(milliseconds: 400);

/// How a call fits what it sends to each connection and to this device's processor, as the
/// connections' statistics tell it, and drops a voice held up by the network.
extension _Fitting on Call {
  /// A shared screen keeps its size, so text stays readable, and drops frames when the
  /// network falls short, unless that leaves it choppy (see [ScreenFit]). A screen larger
  /// than its quality allows goes out scaled down to fit, keeping its shape, and at most
  /// at the quality's frame rate.
  ///
  /// The camera goes out at its step's size (see [cameraStep]), made smaller for a
  /// connection that falls short (see [CameraFit]), and WebRTC changes neither its size
  /// nor its rate. WebRTC would otherwise ask the camera itself for smaller pictures or
  /// fewer frames when one connection or the processor falls short, and the camera gives
  /// every connection the smallest picture and lowest rate any of them asked for: one
  /// slow connection made the camera smaller for everyone. For the same reason the most
  /// frames a second is the same on every connection.
  Future<void> _tune(CallPeer peer, RTCPeerConnection connection) async {
    final transceivers = await _channels(connection);
    final screenScale = _scaleFor(peer);
    final camera = _cameraOut;
    for (final channel in [_camera, _screen]) {
      if (transceivers.length <= channel) continue;
      final sender = transceivers[channel].sender;
      try {
        final parameters = sender.parameters;
        parameters.degradationPreference = channel == _screen
            ? RTCDegradationPreference.MAINTAIN_RESOLUTION
            : RTCDegradationPreference.MAINTAIN_FRAMERATE_AND_RESOLUTION;
        // Only what is set here goes back: the parameters as read carry empty values
        // (no scalability mode, a bitrate of 0) that WebRTC refuses, and then refuses the
        // whole change.
        parameters.encodings = [
          for (final encoding in parameters.encodings ?? <RTCRtpEncoding>[])
            RTCRtpEncoding(
              rid: encoding.rid,
              active: encoding.active,
              numTemporalLayers: null,
              scaleResolutionDownBy: channel == _screen
                  ? screenScale
                  : camera.scale * peer._cameraFit.factor,
              maxFramerate: channel == _screen
                  ? math.min(screenQuality.fps, _screenLoad.limits.fps)
                  : camera.step.fps,
              // Always set: a value left out keeps the one set before.
              maxBitrate: channel == _screen
                  ? _screenKbpsAt(screenScale) * 1000
                  : camera.step.kbps * 1000,
            ),
        ];
        if (!await sender.setParameters(parameters)) {
          _trace('tune: channel $channel refused');
        }
      } catch (e) {
        _trace('tune: $e');
      }
    }
  }

  /// The number of people or the camera's size changed: when that changes how the camera
  /// goes out, every connection is told, without agreeing on anything again.
  void _stepCamera() {
    if (_closed) return;
    final people = _peopleForCamera ?? _people;
    final step = cameraStep(people);
    _cameraLines = math.max(_cameraLines, localCamera.value.height);
    _cameraWidth = math.max(_cameraWidth, localCamera.value.width);
    final out = (
      step: step,
      scale: cameraScale(_cameraLines, step, width: _cameraWidth),
    );
    if (out == _cameraOut) return;
    _cameraOut = out;
    _trace(
      'camera for $people people: ${step.lines ?? 'all'} lines '
      '(scaled by ${out.scale.toStringAsFixed(2)}), ${step.kbps} kbit/s, '
      '${step.fps} fps',
    );
    unawaited(_retune());
  }

  /// The most the shared screen may use going out scaled down by [scale] (see
  /// [screenKbps]), at the rate it goes out at now. Before its size is known it is taken
  /// to fill its quality's 16:9 box, within the ceiling for those watching (1080 lines
  /// when as captured).
  int _screenKbpsAt(double scale) {
    final video = localScreen.value;
    final lines =
        (cappedLines(screenQuality.lines, _screenLoad.ceiling) ?? 1080)
            .toDouble();
    final (width, height) = video.width > 0 && video.height > 0
        ? (video.width / scale, video.height / scale)
        : (lines * 16 / 9, lines);
    return screenKbps(
      width,
      height,
      math.min(screenQuality.fps, _screenLoad.limits.fps),
    );
  }

  /// The sizes, in lines, the shared screen can go out at (see [ScreenFit.sizes]).
  List<int> get _sizes {
    final height = localScreen.value.height;
    return height <= 0
        ? const [0]
        : ScreenFit.sizes((height / _screenScale).round());
  }

  /// How much the shared screen is scaled down for [peer]: to its quality, and further
  /// down by as many steps as its connection needs.
  double _scaleFor(CallPeer peer) {
    final sizes = _sizes;
    var lines = sizes[math.min(peer._fit.step, sizes.length - 1)];
    // And no larger than this device's processor allows for everyone.
    final most = _screenLoad.limits.lines;
    if (most != null && lines > most) lines = most;
    final height = localScreen.value.height;
    return lines <= 0 || height <= 0
        ? _screenScale
        : math.max(_screenScale, height / lines);
  }

  /// Follows how this device's shared screen goes out to [peer], from the statistics of
  /// its connection, and steps its size down or up as [ScreenFit] says. Someone who no
  /// longer watches it starts over.
  void _fitScreen(CallPeer peer, List<StatsReport> reports, DateTime now) {
    final connection = peer.connection;
    if (connection == null) return;
    if (!sharing || peer.watches != me) {
      if (peer._fit.step > 0) {
        peer._fit.reset();
        _tuneSoon(peer, connection);
      }
      peer._fit.reset();
      _showScreenOut(peer.id, null);
      return;
    }
    final track = _screenStream?.getVideoTracks().firstOrNull?.id;
    var lines = 0;
    var width = 0;
    var fps = -1.0;
    var captured = 0.0;
    for (final report in reports) {
      final v = report.values;
      if (report.type == 'outbound-rtp' && '${v['mid']}' == '$_screen') {
        lines = (v['frameHeight'] as num? ?? 0).toInt();
        width = (v['frameWidth'] as num? ?? 0).toInt();
        fps = (v['framesPerSecond'] as num? ?? 0).toDouble();
      } else if (report.type == 'media-source' &&
          v['kind'] == 'video' &&
          v['trackIdentifier'] == track) {
        captured = (v['framesPerSecond'] as num? ?? 0).toDouble();
      }
    }
    // Nothing sent yet, or no frames to send: nothing to judge.
    if (fps < 0 || captured <= 0) return;
    final sizes = _sizes;
    final changed = peer._fit.take(
      fps: fps,
      // What goes out is judged against the rate this device's processor allows now.
      wanted: math.min(
        captured,
        math.min(screenQuality.fps, _screenLoad.limits.fps).toDouble(),
      ),
      steps: sizes.length,
      now: now,
    );
    if (lines > 0) {
      _showScreenOut(peer.id, (
        width: width,
        height: lines,
        fps: fps,
        lowered: peer._fit.step > 0,
        spared: _spared,
      ));
    }
    if (!changed) return;
    _trace(
      'screen to ${peer.id.substring(0, 8)}: '
      '${sizes[math.min(peer._fit.step, sizes.length - 1)]} lines '
      '(was ${lines}p at ${fps.toStringAsFixed(1)} fps of '
      '${captured.toStringAsFixed(1)})',
    );
    _tuneSoon(peer, connection);
  }

  /// Follows, from [connection]'s statistics, whether this device's shared screen has gone
  /// out to [peer] since they started watching it. A capture that gives a picture only
  /// when the screen changes (the portal's on Linux, ScreenCaptureKit on macOS) gives
  /// none to a new watcher's encoder while the screen stays still: their first picture
  /// waited for something on it to move. A watcher to whom nothing has gone out by the
  /// reading after the one that first found them watching gets the capture's last picture
  /// again ([_refreshScreen]) and, from the second time, the screen put on their
  /// connection again, as their closing and opening it does; [screenNudges] times at most
  /// each time they start watching, or on a new connection.
  void _followScreenOut(
    CallPeer peer,
    RTCPeerConnection connection,
    List<StatsReport> reports,
  ) {
    num frames = 0;
    for (final report in reports) {
      final v = report.values;
      if (report.type == 'outbound-rtp' && '${v['mid']}' == '$_screen') {
        frames = v['framesEncoded'] as num? ?? 0;
      }
    }
    final before = peer._screenSent;
    if (!sharing || peer.watches != me) {
      peer._screenSent = (on: connection, frames: frames, watched: false);
      peer._screenNudges = 0;
      return;
    }
    if (before == null || before.on != connection || !before.watched) {
      // Judged from the next reading, against what had gone out before they watched (a
      // new connection's counts start at nothing).
      final had = before != null && before.on == connection ? before.frames : 0;
      peer._screenSent = (on: connection, frames: had, watched: true);
      peer._screenNudges = 0;
      return;
    }
    if (frames > before.frames || peer._screenNudges >= Call.screenNudges) {
      return;
    }
    final nudge = ++peer._screenNudges;
    _trace(
      'screen to ${peer.id.substring(0, 8)}: nothing sent yet; '
      '${nudge > 1 ? 'put on again, and ' : ''}its last picture again',
    );
    peer
        ._inTurn(() async {
          if (_closed ||
              peer.connection != connection ||
              !sharing ||
              peer.watches != me) {
            return;
          }
          if (nudge > 1) {
            final channels = await _channels(connection);
            if (channels.length > _screen) {
              final sender = channels[_screen].sender;
              await sender.replaceTrack(null);
              await sender.replaceTrack(_track(_screen, peer));
            }
          }
          await _refreshScreen();
        })
        .catchError((Object e) => _trace('screen to ${peer.id}: $e'));
  }

  /// Follows what [peer]'s connection can send for this device's camera, from its
  /// statistics, and makes the camera smaller or larger for them as [CameraFit] says. A
  /// camera turned off, or a new connection, starts over.
  void _fitCamera(CallPeer peer, List<StatsReport> reports, DateTime now) {
    final connection = peer.connection;
    if (connection == null) return;
    final fit = peer._cameraFit;
    if (!camera || peer._cameraFitOn != connection) {
      peer._cameraFitOn = connection;
      if (fit.step > 0) {
        fit.reset();
        _tuneSoon(peer, connection);
      }
      fit.reset();
      if (!camera) return;
    }
    Map<dynamic, dynamic>? sent;
    for (final report in reports) {
      if (report.type == 'outbound-rtp' &&
          '${report.values['mid']}' == '$_camera') {
        sent = report.values;
      }
    }
    double? number(String key) => (sent?[key] as num?)?.toDouble();
    // Nothing went out yet: nothing to judge.
    if ((number('framesEncoded') ?? 0) <= 0) return;
    final fps = number('framesPerSecond') ?? 0;
    // What the camera's encoder is given, which is what the connection can send less
    // what goes with it, at most the step's bitrate; or else what the whole connection
    // can send.
    final target = (number('targetBitrate') ?? 0) / 1000;
    final available = availableKbps(reports);
    final kbps = target > 0 ? target : available ?? 0;
    final out = _cameraOut;
    if (!fit.take(kbps: kbps, stepKbps: out.step.kbps, now: now)) return;
    final lines = (_cameraLines > 0 ? _cameraLines : 720) / out.scale;
    _trace(
      'camera to ${peer.id.substring(0, 8)}: ${(lines / fit.factor).round()} lines, '
      '${fit.factor}x below its step (was ${number('frameWidth')?.round()}x'
      '${number('frameHeight')?.round()} at ${fps.toStringAsFixed(1)} fps; '
      'target ${target.round()} kbit/s of ${out.step.kbps}, '
      'can send ${available?.round()})',
    );
    _tuneSoon(peer, connection);
  }

  /// Tunes [peer]'s connection after what is under way on it, without waiting for that.
  /// Lowers or raises the shared screen for everyone as [ScreenLoad] says, from a reading
  /// of every watcher's encoder.
  void _fitScreenLoad(
    Map<
      String,
      ({bool cpu, double encodeSeconds, double? fps, double? captured})
    >
    encoders,
  ) {
    final before = _screenLoad.level;
    _screenLoad.capturedFps = _capturedAt > 0
        ? math.min(screenQuality.fps, _capturedAt)
        : screenQuality.fps;
    final ceiling = _screenLoad.ceiling;
    // Also where the ceiling goes back up once fewer have watched for a while.
    final more = _screenLoad.watchersAre(watchers);
    final changed = _screenLoad.take(encoders: encoders, now: DateTime.now());
    if (!more && !changed) return;
    _screenLoadChanged(before, ceiling);
  }

  /// Someone started or stopped watching this device's shared screen: the screen's
  /// ceiling and level follow their number at once (see [ScreenLoad.watchersAre]), before
  /// a new watcher's encoder starts, rather than at the next reading. Those who watch are
  /// tuned after what is under way on their connections, all but [inTurn], whose turn this
  /// is: its caller tunes it.
  void _watchersChanged(CallPeer inTurn) {
    if (!sharing) return;
    final before = _screenLoad.level;
    final ceiling = _screenLoad.ceiling;
    if (_screenLoad.watchersAre(watchers)) {
      _screenLoadChanged(before, ceiling, except: inTurn);
    }
  }

  /// The screen's level or ceiling changed from [before] and [ceiling]: its scale follows,
  /// and every connection watching is tuned again, but [except].
  void _screenLoadChanged(int before, int? ceiling, {CallPeer? except}) {
    final limits = _screenLoad.limits;
    _scaleAgain();
    _trace(
      'screen for everyone: level ${_screenLoad.level} (was $before), '
      '${limits.lines ?? 'full'} lines at most ${limits.fps} fps, within '
      '${_screenLoad.ceiling ?? 'any'} lines (was ${ceiling ?? 'any'}), '
      '$watchers watching',
    );
    for (final peer in peers.values) {
      final connection = peer.connection;
      if (connection != null && peer.watches == me && peer != except) {
        _tuneSoon(peer, connection);
      }
    }
  }

  void _tuneSoon(CallPeer peer, RTCPeerConnection connection) {
    peer
        ._inTurn(() async {
          if (!_closed && peer.connection == connection) {
            await _tune(peer, connection);
          }
        })
        .catchError((Object e) => _trace('tune: $e'));
  }

  /// Every person's screen fit starts over, as for a new share; one stepped down goes back
  /// to the full size. Those already watching (another screen picked while they watch)
  /// set where it starts at once, as their number calls for (see [ScreenLoad.startFor]).
  Future<void> _resetFits() async {
    var stepped = _screenLoad.level > 0;
    _screenLoad.reset();
    if (sharing) _screenLoad.watchersAre(watchers);
    for (final peer in peers.values) {
      stepped |= peer._fit.step > 0;
      peer._fit.reset();
    }
    stepped |= _scaleAgain();
    if (stepped) await _retune();
  }

  /// How much the shared screen is scaled down now, from its captured size and quality,
  /// within the ceiling for the number watching (see [screenCeiling]).
  double _scaleNow() =>
      _scaleTo(cappedLines(screenQuality.lines, _screenLoad.ceiling));

  /// How much the shared screen is scaled down to fit the 16:9 box of [lines] (null: as
  /// captured), from its captured size.
  double _scaleTo(int? lines) {
    final video = localScreen.value;
    final (width, height) = video.width > 0 && video.height > 0
        ? (video.width, video.height)
        : (_screenTooLarge?.width ?? 0.0, _screenTooLarge?.height ?? 0.0);
    return width <= 0 || height <= 0
        ? 1.0
        : screenScale(width, height, lines: lines);
  }

  /// Whether the shared screen goes out smaller or slower than chosen to everyone, to
  /// spare this device (see [ScreenOut]).
  bool get _spared =>
      _screenLoad.lowered || _screenScale > _scaleTo(screenQuality.lines);

  /// The shared screen's size became known or changed: every connection is told the new
  /// scale, and the bitrate the new size may use (see [screenKbps]).
  Future<void> _rescale() async {
    if (_scaleAgain()) await _retune();
  }

  /// Takes the scale and bitrate the shared screen goes out at now; returns whether
  /// either changed, and the connections are to be told.
  bool _scaleAgain() {
    // A size not known (no picture yet, or none any more) keeps the scale there is: while
    // another screen is picked, the one before may still send to those who watch it.
    final video = localScreen.value;
    final known = video.width > 0 && video.height > 0;
    if (!known && _screenTooLarge == null) return false;
    final scale = _scaleNow();
    final kbps = _screenKbpsAt(scale);
    if (_closed || (scale == _screenScale && kbps == _screenKbps)) return false;
    _screenScale = scale;
    _screenKbps = kbps;
    return true;
  }

  /// Tells every connection how the camera and the screen go out now.
  Future<void> _retune() async {
    if (_closed) return;
    for (final peer in peers.values.toList()) {
      try {
        await peer._inTurn(() async {
          final connection = peer.connection;
          if (connection != null) await _tune(peer, connection);
        });
      } catch (e) {
        _trace('retune: $e');
      }
    }
  }

  /// After a stall of the network, the voice sent meanwhile may have waited in a queue on
  /// the way and comes all at once; WebRTC then plays all of it, seconds late, and only
  /// slowly catches up (over 8 to 10 s in the lab). When a reading with nothing
  /// of someone's voice (their microphone on) is followed by one where their voice waits
  /// [lateVoice] longer than WebRTC aims for, their voice is not taken for a moment,
  /// which empties what WebRTC holds of it, and taken again: about half a second of
  /// silence instead.
  void _flushLateVoice(CallPeer peer, List<StatsReport> reports) {
    final v = reports
        .where(
          (r) => r.type == 'inbound-rtp' && '${r.values['mid']}' == '$_audio',
        )
        .firstOrNull
        ?.values;
    if (v == null) return;
    num n(String key) => v[key] as num? ?? 0;
    final now = (
      packets: n('packetsReceived'),
      delay: n('jitterBufferDelay'),
      target: n('jitterBufferTargetDelay'),
      emitted: n('jitterBufferEmittedCount'),
    );
    final before = peer._voiceIn;
    peer._voiceIn = now;
    if (before == null || peer._voicePaused) return;
    final emitted = now.emitted - before.emitted;
    if (now.packets == before.packets) {
      // What waited comes over the next seconds, not always at once.
      if (peer.mic) peer._voiceGap = 5;
      return;
    }
    if (peer._voiceGap == 0 || emitted <= 0) return;
    peer._voiceGap--;
    final late =
        (now.delay - before.delay - (now.target - before.target)) / emitted;
    if (late * 1000 < lateVoice.inMilliseconds) return;
    peer._voiceGap = 0;
    _trace(
      'voice from ${peer.id.substring(0, 8)} ${(late * 1000).round()} ms late: '
      'dropped',
    );
    unawaited(
      _pauseVoiceIn(peer).catchError((Object e) => _trace('voice in: $e')),
    );
  }

  Future<void> _pauseVoiceIn(CallPeer peer) async {
    try {
      await _agreeVoiceIn(peer, paused: true);
    } finally {
      // Never left out, even if the first agreement failed.
      await _agreeVoiceIn(peer, paused: false);
    }
  }

  /// Takes their voice or not, and agrees on it with them.
  Future<void> _agreeVoiceIn(CallPeer peer, {required bool paused}) async {
    peer._voicePaused = paused;
    final transceivers = await _channels(peer.connection);
    if (transceivers.isEmpty || peers[peer.id] != peer || _closed) return;
    final agreed = peer._agreed = Completer<void>();
    await transceivers[_audio].setDirection(_voiceDirection(peer));
    await _renegotiate(peer);
    await agreed.future.timeout(const Duration(seconds: 5), onTimeout: () {});
  }
}
