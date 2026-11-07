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

  /// Closes the connection with [peer], which sent [what] (see [fairPicture]), and makes
  /// no other while it stays in the call: a modified copy could keep this device decoding
  /// pictures of any size. Every picture its renderers get is checked
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
        reports = await connection.getStats();
      } catch (_) {
        continue;
      }
      if (_closed || peers[peer.id] != peer) continue;
      final unfair = unfairPicture(reports);
      if (unfair != null) {
        await _keepOut(peer, unfair);
        continue;
      }
      final now = DateTime.now();
      final before = quality.linkOf(peer.id);
      quality.take(peer.id, LinkSample.of(reports, voiceMid: '$_audio'), now);
      _fitScreen(peer, reports, now);
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

  /// Who talks only shows on screen: nothing is read while the window is hidden, and this
  /// device's own level not while its microphone is off.
  Future<void> _readLevels() async {
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
        // The loudest report counts: after the other side stops and starts sending again,
        // its old silent stream may still be listed beside the new one. This device's own
        // level is the same on every connection, so it is read from the first one only.
        // Voices only: the sound of a shared screen is not someone talking.
        final mic = _micStream?.getAudioTracks().firstOrNull?.id;
        for (final report in await connection.getStats()) {
          final level = report.values['audioLevel'];
          if (level is! num) continue;
          final values = report.values;
          if (report.type == 'inbound-rtp' &&
              (values['mid'] == null || values['mid'] == '$_audio')) {
            remote = math.max(remote, level.toDouble());
          }
          if (!localRead &&
              report.type == 'media-source' &&
              (mic == null ||
                  values['trackIdentifier'] == null ||
                  values['trackIdentifier'] == mic)) {
            local = math.max(local, level.toDouble());
          }
        }
        localRead = true;
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
}
