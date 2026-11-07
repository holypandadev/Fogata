/// Videos with each system's own player: AVFoundation on macOS, Media Foundation on
/// Windows and GStreamer on Linux (loaded when needed, so the application runs without it).
/// Reads a video's first picture and length, and plays it into a [Texture].
library;

import 'dart:math' as math;

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';

const _channel = MethodChannel('fogata/video');

/// One way a video's first bytes may be: [bytes] where [mask] has 0xFF, anything where it
/// has 0.
typedef VideoHead = ({Uint8List bytes, Uint8List mask});

/// A head with [parts] (bytes by where they start) and anything between them.
VideoHead _head(Map<int, List<int>> parts) {
  final size = parts.entries
      .map((e) => e.key + e.value.length)
      .reduce(math.max);
  final bytes = Uint8List(size);
  final mask = Uint8List(size);
  for (final MapEntry(key: at, value: part) in parts.entries) {
    bytes.setAll(at, part);
    mask.fillRange(at, at + part.length, 0xFF);
  }
  return (bytes: bytes, mask: mask);
}

/// How a file named [name] may begin to be the kind of video its name says: MP4 and
/// QuickTime (`.mp4`, `.m4v`, `.mov`: a box such as `ftyp` or `moov`), Matroska and WebM
/// (`.mkv`, `.webm`: EBML's 1A 45 DF A3) or AVI (`.avi`: `RIFF` then `AVI `). None for
/// anything else, which is neither read nor played: each system's player picks what reads
/// a file by its contents or its name, and either way it should be one of these.
///
/// The player checks them against the bytes of the file it opened itself, once, and reads
/// the video from that same open file: nothing can be put in its place between the check
/// and the reading.
@visibleForTesting
List<VideoHead> videoHeads(String name) {
  final dot = name.lastIndexOf('.');
  final ext = dot < 0 ? '' : name.substring(dot + 1).toLowerCase();
  return switch (ext) {
    'mp4' || 'm4v' || 'mov' => [
      for (final box in const [
        'ftyp',
        'moov',
        'mdat',
        'wide',
        'free',
        'skip',
        'pnot',
      ])
        _head({4: box.codeUnits}),
    ],
    'mkv' || 'webm' => [
      _head({
        0: const [0x1A, 0x45, 0xDF, 0xA3],
      }),
    ],
    'avi' => [
      _head({0: 'RIFF'.codeUnits, 8: 'AVI '.codeUnits}),
    ],
    _ => const [],
  };
}

/// Whether [head], a file's first bytes, is one of [heads], as the player checks it.
bool headMatches(List<VideoHead> heads, List<int> head) => heads.any(
  (h) =>
      head.length >= h.bytes.length &&
      [for (var i = 0; i < h.bytes.length; i++) i]
          .every((i) => head[i] & h.mask[i] == h.bytes[i] & h.mask[i]),
);

/// Whether a file named [name] whose first bytes are [head] is the kind of video its name
/// says ([videoHeads]).
@visibleForTesting
bool videoHeadMatches(String name, List<int> head) =>
    headMatches(videoHeads(name), head);

/// [videoHeads] as the player is given them.
List<Map<String, Uint8List>>? _headsFor(String path) {
  final heads = videoHeads(path);
  if (heads.isEmpty) return null;
  return [
    for (final h in heads) {'bytes': h.bytes, 'mask': h.mask},
  ];
}

/// What a video looks like: its first picture as a small JPEG, its size and how long it
/// lasts.
class VideoProbe {
  const VideoProbe({
    required this.thumb,
    required this.width,
    required this.height,
    required this.durationMs,
  });

  final Uint8List thumb;
  final int width;
  final int height;
  final int durationMs;
}

/// Reads a video's first picture, at most [maxWidth] pixels wide, its size and length.
/// Null when the system cannot read it (an unknown format, or no player on Linux), or when
/// it is not the kind of video its name says ([videoHeads]).
Future<VideoProbe?> probeVideo(String path, {int maxWidth = 480}) async {
  final heads = _headsFor(path);
  if (heads == null) return null;
  try {
    final r = await _channel.invokeMapMethod<String, Object?>('probe', {
      'path': path,
      'heads': heads,
      'maxWidth': maxWidth,
    });
    final thumb = r?['thumb'];
    if (r == null || thumb is! Uint8List) return null;
    return VideoProbe(
      thumb: thumb,
      width: r['width']! as int,
      height: r['height']! as int,
      durationMs: r['durationMs']! as int,
    );
  } on PlatformException {
    return null;
  } on MissingPluginException {
    return null;
  }
}

/// Where a player stands.
class VideoState {
  const VideoState({
    this.positionMs = 0,
    this.durationMs = 0,
    this.playing = false,
    this.ended = false,
    this.width = 0,
    this.height = 0,
    this.error,
  });

  final int positionMs;
  final int durationMs;
  final bool playing;
  final bool ended;
  final int width;
  final int height;

  /// Why it cannot play, if it cannot.
  final String? error;
}

/// A video playing into a texture, shown with [Texture] by [textureId].
class VideoPlayer {
  VideoPlayer._(this.textureId);

  final int textureId;
  bool _closed = false;

  /// Opens a video, paused at its start. Null when the system cannot play it, or when it
  /// is not the kind of video its name says ([videoHeads]). Its sound goes to [output] (an
  /// output device as the voice engine names it) on macOS and Windows, while that is
  /// connected; on Linux the application moves it there.
  static Future<VideoPlayer?> open(String path, {String? output}) async {
    final heads = _headsFor(path);
    if (heads == null) return null;
    try {
      final id = await _channel.invokeMethod<int>('open', {
        'path': path,
        'heads': heads,
        'output': ?output,
      });
      return id == null ? null : VideoPlayer._(id);
    } on PlatformException {
      return null;
    } on MissingPluginException {
      return null;
    }
  }

  Future<void> _call(
    String method, [
    Map<String, Object?> args = const {},
  ]) async {
    if (_closed) return;
    try {
      await _channel.invokeMethod<void>(method, {'id': textureId, ...args});
    } on PlatformException {
      // A player that went away answers no more.
    }
  }

  Future<void> play() => _call('play');
  Future<void> pause() => _call('pause');
  Future<void> seek(int ms) => _call('seek', {'ms': ms < 0 ? 0 : ms});

  /// From 0 to 1.
  Future<void> setVolume(double volume) =>
      _call('volume', {'volume': volume.clamp(0.0, 1.0)});

  Future<VideoState> state() async {
    if (_closed) return const VideoState();
    try {
      final r = await _channel.invokeMapMethod<String, Object?>('state', {
        'id': textureId,
      });
      if (r == null) return const VideoState();
      return VideoState(
        positionMs: r['positionMs'] as int? ?? 0,
        durationMs: r['durationMs'] as int? ?? 0,
        playing: r['playing'] as bool? ?? false,
        ended: r['ended'] as bool? ?? false,
        width: r['width'] as int? ?? 0,
        height: r['height'] as int? ?? 0,
        error: r['error'] as String?,
      );
    } on PlatformException {
      return const VideoState();
    }
  }

  /// Stops it and lets the texture go.
  Future<void> close() async {
    if (_closed) return;
    await _call('close');
    _closed = true;
  }
}

/// The picture of a [VideoPlayer], filling its box.
class VideoView extends StatelessWidget {
  const VideoView({super.key, required this.player});
  final VideoPlayer player;

  @override
  Widget build(BuildContext context) => Texture(textureId: player.textureId);
}
