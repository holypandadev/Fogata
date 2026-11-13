part of '../call.dart';

/// Whether a picture received on [channel] (camera or screen) of `width` by `height` is
/// one a participant would send. WebRTC decodes whatever size the sender chose, and
/// makes room for it, before anything is drawn.
/// A camera's picture scaled down to fit may come a few pixels larger (the encoder's
/// rounding and alignment): that much more is let through. A screen's is held to the
/// decoder's own limit, with nothing more: a larger one is never decoded and shows black,
/// so it keeps its sender out with a note rather than leave a black picture unexplained.
@visibleForTesting
bool fairPicture(int channel, int width, int height) {
  return switch (channel) {
    _camera =>
      width <= maxCameraSide + 16 &&
          height <= maxCameraSide + 16 &&
          width * height <= maxCameraPixels * 1.02,
    _screen =>
      width <= maxScreenSide &&
          height <= maxScreenSide &&
          width * height <= maxScreenPixels,
    _ => false,
  };
}

/// What, in a connection's statistics, arrives larger than [fairPicture] allows, as words
/// for the log; null when nothing does.
@visibleForTesting
String? unfairPicture(List<StatsReport> reports) {
  for (final report in reports) {
    final v = report.values;
    if (report.type != 'inbound-rtp' || v['kind'] != 'video') continue;
    final width = (v['frameWidth'] as num?)?.toInt() ?? 0;
    final height = (v['frameHeight'] as num?)?.toInt() ?? 0;
    if (width <= 0 && height <= 0) continue;
    // A copy whose statistics name no channel (older WebRTC) is held to the larger
    // limit, a screen's, rather than kept out for every picture.
    final mid = v['mid'];
    final channel = mid == null ? _screen : int.tryParse('$mid') ?? -1;
    if (!fairPicture(channel, width, height)) {
      return 'video on channel ${v['mid']} at ${width}x$height';
    }
  }
  return null;
}

/// How fast each channel's pictures arrive on one connection, reading after reading, to
/// tell a participant who sends far more of them than any copy does: a modified copy could
/// keep every receiver decoding its camera or its screen at hundreds of frames a second,
/// within the size [fairPicture] allows, whether anyone watches or not. The limit of each
/// reading is [room] times what honest copies send, beta.7 on: at most [mostFps] frames a
/// second on either channel (the camera's step for two people, see [cameraStep], and the
/// screen's highest quality, [ScreenQuality.rates]), and at most [cameraMaxKbps] or
/// [screenMaxKbps] (what this device's own description lets each send, which every copy's
/// encoder keeps to). Older copies judge newer ones by these: a later version that sends
/// faster must stay well within them. Frames are counted as
/// decoded: after a stall, WebRTC skips the frames that came late rather than decode them
/// all, so catching up does not show as a burst. Bytes are counted as received, which a
/// key frame or a network catching up after a stall can push past the limit for one
/// reading, never for two in a row.
@visibleForTesting
class VideoFlood {
  /// The most frames a second any copy sends of its camera or its screen.
  static const mostFps = 60;

  /// How far above what any copy sends a reading may go.
  static const room = 3;

  /// Readings in a row above it before the sender is kept out.
  static const readings = 2;

  /// A reading closer than this to the one before is not judged (one taken 1.1 s after
  /// another for a silent participant is); it counts with the next.
  static const leastSpan = Duration(milliseconds: 900);

  final _last = <String, ({num frames, num bytes})>{};
  DateTime? _at;
  final _over = <int, int>{};

  /// The most kbit/s any copy sends on [channel].
  static int mostKbps(int channel) =>
      channel == _camera ? cameraMaxKbps : screenMaxKbps;

  /// Takes the connection's statistics read at [now]: what has arrived above the limits for
  /// [readings] readings in a row, as words for the log; null otherwise.
  String? take(List<StatsReport> reports, DateTime now) {
    final at = _at;
    if (at != null && now.difference(at) < leastSpan) return null;
    final seconds = at == null
        ? 0.0
        : now.difference(at).inMicroseconds / Duration.microsecondsPerSecond;
    final frames = <int, num>{};
    final bytes = <int, num>{};
    final counts = <String, ({num frames, num bytes})>{};
    for (final report in reports) {
      final v = report.values;
      if (report.type != 'inbound-rtp' || v['kind'] != 'video') continue;
      // A copy whose statistics name no channel (older WebRTC) is held to the larger
      // limits, a screen's, as in [unfairPicture].
      final mid = v['mid'];
      final channel = mid == null ? _screen : int.tryParse('$mid') ?? -1;
      final count = (
        frames: v['framesDecoded'] as num? ?? 0,
        bytes: v['bytesReceived'] as num? ?? 0,
      );
      counts[report.id] = count;
      final before = _last[report.id];
      // A stream's counts start again only with a new stream: one that went down is
      // not judged against the old.
      if (before == null ||
          count.frames < before.frames ||
          count.bytes < before.bytes) {
        continue;
      }
      frames[channel] = (frames[channel] ?? 0) + count.frames - before.frames;
      bytes[channel] = (bytes[channel] ?? 0) + count.bytes - before.bytes;
    }
    _last
      ..clear()
      ..addAll(counts);
    _at = now;
    if (seconds <= 0) return null;
    for (final channel in {..._over.keys, ...frames.keys}) {
      final fps = (frames[channel] ?? 0) / seconds;
      final kbps = (bytes[channel] ?? 0) * 8 / 1000 / seconds;
      final over = fps > mostFps * room || kbps > mostKbps(channel) * room;
      final inRow = _over[channel] = over ? (_over[channel] ?? 0) + 1 : 0;
      if (inRow >= readings) {
        return 'video on channel $channel at ${fps.round()} frames a second, '
            '${kbps.round()} kbit/s';
      }
    }
    return null;
  }
}

/// What a connection's statistics say of the shared screen's encoder, for [ScreenLoad]:
/// whether WebRTC holds it back for the processor, its time spent encoding so far, in
/// seconds, the frames a second it sends and those the screen is captured at (each null
/// when not told); null while the screen does not go out on it.
@visibleForTesting
({bool cpu, double encodeSeconds, double? fps, double? captured})?
screenEncoder(List<StatsReport> reports) {
  for (final report in reports) {
    final v = report.values;
    if (report.type != 'outbound-rtp' || '${v['mid']}' != '$_screen') continue;
    if ((v['framesEncoded'] as num? ?? 0) <= 0) return null;
    // What the screen is captured at now: its media source, the same for every watcher.
    final source = reports
        .where((r) => r.type == 'media-source' && r.id == v['mediaSourceId'])
        .firstOrNull;
    return (
      cpu: v['qualityLimitationReason'] == 'cpu',
      encodeSeconds: (v['totalEncodeTime'] as num? ?? 0).toDouble(),
      fps: (v['framesPerSecond'] as num?)?.toDouble(),
      captured: (source?.values['framesPerSecond'] as num?)?.toDouble(),
    );
  }
  return null;
}

/// What a call reads from its connections' statistics every so often: how what each one
/// sends arrives, who sends pictures larger than any participant would, and who is
/// talking.
extension _Measuring on Call {
  /// What tells the others how this device's own network is: its name, or null while it
  /// cannot be told apart from the connections. Older versions ignore it.
  Map<String, dynamic> get _net => {'t': 'net', 'l': quality.network?.name};

  /// Reads every connection's statistics, or only those of [only], into [quality]; while
  /// in a call, also with the window hidden, since the others hear of this device's own
  /// network from it. Someone silent for 2 s is read again a second later, so losing
  /// them shows when nothing has come for 3 s rather than 4.
  Future<void> _measure([Set<String>? only]) async {
    // A reading that takes long is not overtaken by the next one.
    if (_reading) return;
    _reading = true;
    try {
      await _measureNow(only);
    } finally {
      _reading = false;
    }
  }

  /// Closes the connection with [peer], which sent [what] (see [fairPicture] and
  /// [VideoFlood]), and makes no other while it stays in the call: a modified copy could
  /// keep this device decoding pictures of any size, at any rate. Every picture its renderers get is checked
  /// ([pictureArrived]), and every reading of the statistics, every 2 s.
  Future<void> _keepOut(CallPeer peer, String what) async {
    if (peer._keptOut) {
      // Told already; a connection made meanwhile goes too.
      await peer._close();
      await peer._closeSoundIn();
      return;
    }
    peer._keptOut = true;
    _trace('${peer.id.substring(0, 8)} sent $what: kept out of the call');
    if (peer.rendering) {
      peer.camera.srcObject = null;
      peer.screen.srcObject = null;
    }
    if (watching == peer.id) watch(null);
    if (focused == peer.id) focus(null);
    // Not reconnecting: neither they nor this device's own network show as such.
    quality.forget(peer.id);
    await peer._close();
    await peer._closeSoundIn();
    // Nor does this device's screen sound go to them.
    await _soundTo(peer);
    _report(l10n.callKeptOut(nameOf(peer.id)));
  }

  Future<void> _measureNow(Set<String>? only) async {
    final quiet = <String>{};
    final encoders =
        <
          String,
          ({bool cpu, double encodeSeconds, double? fps, double? captured})
        >{};
    var voiceSending = false;
    double? voiceRead;
    final voiceSent = <String, num>{};
    for (final peer in peers.values.toList()) {
      if (only != null && !only.contains(peer.id)) continue;
      final connection = peer.connection;
      if (connection == null || !peer.connected) {
        quality.cut(peer.id);
        continue;
      }
      final List<StatsReport> reports;
      try {
        reports = await _stats(connection);
      } catch (_) {
        continue;
      }
      if (_closed || peers[peer.id] != peer) continue;
      final now = DateTime.now();
      final unfair =
          unfairPicture(reports) ?? peer._pictures.take(reports, now);
      if (unfair != null) {
        await _keepOut(peer, unfair);
        continue;
      }
      final before = quality.linkOf(peer.id);
      quality.take(peer.id, LinkSample.of(reports, voiceMid: '$_audio'), now);
      _fitScreen(peer, reports, now);
      // Readings again of the quiet only would count faster.
      if (only == null) {
        _followScreenOut(peer, connection, reports);
        _awaitPicture(peer, connection);
      }
      // What the microphone gives and sends, on a connection agreed to send voice.
      if (mic && await _sendsVoice(connection)) {
        voiceSending = true;
        for (final r in reports) {
          final v = r.values;
          if (r.type == 'outbound-rtp' && '${v['mid']}' == '$_audio') {
            final sent = v['packetsSent'];
            if (sent is num) {
              voiceSent['${peer.id}/${identityHashCode(connection)}'] = sent;
            }
          } else if (r.type == 'media-source' && v['kind'] == 'audio') {
            final read = v['totalSamplesDuration'];
            if (read is num) {
              voiceRead = math.max(voiceRead ?? 0, read.toDouble());
            }
          }
        }
      }
      if (sharing && peer.watches == me) {
        final encoder = screenEncoder(reports);
        if (encoder != null) encoders[peer.id] = encoder;
      }
      _fitCamera(peer, reports, now);
      _traceVoice(peer, reports);
      _flushLateVoice(peer, reports);
      final after = quality.linkOf(peer.id);
      if (after != before) {
        _trace('connection to ${peer.id.substring(0, 8)}: ${after?.word}');
      }
      final silence = quality.silenceOf(peer.id, now) ?? Duration.zero;
      if (silence >= const Duration(seconds: 2) &&
          quality.linkOf(peer.id) != NetLevel.lost) {
        quiet.add(peer.id);
      }
    }
    if (_closed) return;
    if (only == null && sharing) _fitScreenLoad(encoders);
    if (only == null &&
        _micCheck.take(
          on: mic && _micStream != null,
          sending: voiceSending,
          read: voiceRead,
          sent: voiceSent,
          now: DateTime.now(),
        )) {
      unawaited(_tellMicUnusable());
    }
    final network = quality.network;
    if (network != _told) {
      _told = network;
      for (final peer in peers.values) {
        if (peer.connected) _signal(peer.id, _net);
      }
    }
    if (only == null && quiet.isNotEmpty) {
      Future<void>.delayed(const Duration(milliseconds: 1100), () {
        if (!_closed) _measure(quiet);
      });
    }
  }

  /// With the call's log on, the voice each way with [peer] every 2 s: what the microphone
  /// reads, what goes out, what comes in and what is played, so a voice that is lost on
  /// the way can be told from one never sent.
  void _traceVoice(CallPeer peer, List<StatsReport> reports) {
    if (!_verbose) return;
    final parts = <String>[];
    for (final r in reports) {
      final v = r.values;
      if (r.type == 'media-source' && v['kind'] == 'audio') {
        parts.add(
          'mic level ${v['audioLevel']} read ${v['totalSamplesDuration']}',
        );
      } else if (r.type == 'outbound-rtp' && '${v['mid']}' == '$_audio') {
        parts.add('out ${v['packetsSent']} packets ${v['bytesSent']} bytes');
      } else if (r.type == 'inbound-rtp' && '${v['mid']}' == '$_audio') {
        parts.add(
          'in ${v['packetsReceived']} packets level ${v['audioLevel']} '
          'played ${v['totalSamplesReceived']} concealed ${v['concealedSamples']}',
        );
      }
    }
    _trace(
      'voice with ${peer.id.substring(0, 8)}: mic $mic, ${parts.join('; ')}',
    );
  }

  /// Who talks, every 600 ms. A reading that takes longer (many participants, a busy
  /// computer: one report of statistics each) is not overtaken by the next one, so
  /// readings never pile up.
  Future<void> _readLevels() async {
    if (_readingLevels) return;
    _readingLevels = true;
    try {
      await _readLevelsNow();
    } finally {
      _readingLevels = false;
    }
  }

  /// Who talks only shows on screen: nothing is read while the window is hidden, and this
  /// device's own level not while its microphone is off.
  Future<void> _readLevelsNow() async {
    if (!windowShown.value) {
      unawaited(_alone.close());
      return;
    }
    var changed = false;
    var local = 0.0;
    var localRead = !mic;
    // A copy: peers can come and go while the stats are read.
    for (final peer in peers.values.toList()) {
      final connection = peer.connection;
      if (connection == null || !peer.connected) continue;
      // Someone muted is not talking; their statistics are read only for this device's
      // own level, if still needed.
      if (!peer.mic && localRead) {
        if (peer.speaking) {
          peer.speaking = false;
          changed = true;
        }
        continue;
      }
      var remote = 0.0;
      try {
        // Each level from the statistics of what carries it alone: their voice's receiver,
        // and this device's microphone where it goes out. This device's own level is the
        // same on every connection, so it is read from the first one only. A whole report
        // (every channel, candidate and codec of the connection) is read only when the
        // narrow one cannot tell: the track not known yet, or nothing of it listed yet.
        if (peer.mic) remote = await _voiceLevelOf(peer, connection);
        if (!localRead) {
          local = await _micLevelOn(connection);
          localRead = true;
        }
      } catch (_) {
        continue;
      }
      final speaking = peer.mic && remote > _speaking;
      if (speaking != peer.speaking) {
        peer.speaking = speaking;
        changed = true;
      }
    }
    // Alone in the call, no connection says how loud this device is: a loop of its own
    // does. It goes once someone connects or the microphone is off.
    if (!_closed && !localRead && !peers.values.any((p) => p.connected)) {
      local = await _alone.read(_micStream);
      // The call ended while it opened: its loop goes with it.
      if (_closed) unawaited(_alone.close());
    } else {
      unawaited(_alone.close());
    }
    final speaking = mic && local > _speaking;
    if (speaking != this.speaking) {
      this.speaking = speaking;
      changed = true;
    }
    if (changed) talking.value++;
  }

  /// How loud [peer]'s voice arrives on [connection] ([voiceLevel]): from the statistics
  /// of its receiver alone (WebRTC filters them by the receiver's own stream), or of the
  /// whole connection while that one is not known or lists nothing of the voice yet.
  Future<double> _voiceLevelOf(
    CallPeer peer,
    RTCPeerConnection connection,
  ) async {
    final voice = peer._voice;
    if (voice != null && voice.on == connection) {
      try {
        final level = voiceLevel(await _stats(connection, voice.track));
        if (level != null) return level;
      } catch (_) {
        // Not found (the connection made again meanwhile): the whole report says.
      }
    }
    return voiceLevel(await _stats(connection)) ?? 0;
  }

  /// How loud this device's microphone is, read on [connection] ([micLevel]): from the
  /// statistics of the sender it goes out on alone, or of the whole connection while that
  /// one tells nothing (it sends nothing yet).
  Future<double> _micLevelOn(RTCPeerConnection connection) async {
    final mic = _micStream?.getAudioTracks().firstOrNull;
    if (mic != null) {
      try {
        final level = micLevel(await _stats(connection, mic), mic.id);
        if (level != null) return level;
      } catch (_) {
        // Not on this connection's sender (not put on it yet): the whole report says.
      }
    }
    return micLevel(await _stats(connection), mic?.id) ?? 0;
  }
}

/// How loud a participant's voice arrives, from a connection's statistics or its voice
/// receiver's; null when they list none. The loudest report counts: after the other side
/// stops and starts sending again, its old silent stream may still be listed beside the
/// new one. Voices only: the sound of a shared screen is not someone talking.
@visibleForTesting
double? voiceLevel(List<StatsReport> reports) {
  double? level;
  for (final report in reports) {
    final values = report.values;
    final value = values['audioLevel'];
    if (value is! num || report.type != 'inbound-rtp') continue;
    if (values['mid'] != null && values['mid'] != '$_audio') continue;
    level = math.max(level ?? 0, value.toDouble());
  }
  return level;
}

/// How loud this device's microphone ([mic], its track's id, when known) is, from a
/// connection's statistics or its microphone sender's; null when they list none.
@visibleForTesting
double? micLevel(List<StatsReport> reports, String? mic) {
  double? level;
  for (final report in reports) {
    final values = report.values;
    final value = values['audioLevel'];
    if (value is! num || report.type != 'media-source') continue;
    final track = values['trackIdentifier'];
    if (mic != null && track != null && track != mic) continue;
    level = math.max(level ?? 0, value.toDouble());
  }
  return level;
}
