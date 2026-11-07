part of '../call.dart';

/// The sound of a shared screen: the silent input that lets it go out in the call's own
/// channel while the microphone is off, and the connection of its own it takes to each
/// watcher where it cannot go in that channel.
extension _ScreenSound on Call {
  /// Points WebRTC at the silent input while the microphone is off and the screen's
  /// sound should go out; before any channel starts sending it.
  Future<void> _quietInput() async {
    if (!_wantSilence) return;
    if (_silent) {
      // Still there from an earlier share; made again if it went away meanwhile.
      if (await Call._ownSilenceId() != null) return;
      _silent = false;
    }
    if (!await Call._silentInput(true)) return;
    // The new input takes a moment to be listed.
    for (var i = 0; i < 30 && !_silent; i++) {
      final id = await Call._ownSilenceId();
      if (id == null) {
        await Future<void>.delayed(const Duration(milliseconds: 100));
        continue;
      }
      try {
        await Helper.selectAudioInput(id);
        _silent = true;
      } catch (e) {
        _trace('silent input $id: $e');
        break;
      }
    }
    if (!_silent) await Call._silentInput(false);
  }

  /// Points WebRTC back at the microphone, and removes the silent input, only when the
  /// microphone is turned on or the call has ended. A channel may keep sending a while
  /// after the screen's sound stops (until the other side agrees again), and while WebRTC
  /// records, choosing another input opens it at once; a recording whose input goes away
  /// moves to the system's default input by itself.
  Future<void> _loudInput() async {
    if (!_silent || (!mic && !_closed)) return;
    await Call._select(
      input ?? await Call._first('audioinput'),
      Helper.selectAudioInput,
    );
    _silent = false;
    await Call._silentInput(false);
  }

  // ----- A screen's sound on a connection of its own -----

  /// Opens or closes the connection that carries this device's screen sound to [peer]:
  /// open while they watch. This side always offers it, so offers never cross.
  Future<void> _soundTo(CallPeer peer) => peer._soundTurn = peer._soundTurn
      .then((_) => _soundToNow(peer))
      .catchError((Object e) => _trace('screen sound to ${peer.id}: $e'));

  Future<void> _soundToNow(CallPeer peer) async {
    final want =
        _soundConnections &&
        !_closed &&
        peers[peer.id] == peer &&
        !peer._keptOut &&
        sharingSound &&
        peer.watches == me;
    final open = peer._soundOut;
    if (!want) {
      if (open == null) return;
      peer._soundOut = null;
      await open.close();
      _signal(peer.id, {'t': 'sound-end'});
      return;
    }
    if (open != null) return;
    final connection = await createPeerConnection({
      ..._network(peer.id),
      'fogataScreenSound': true,
    });
    // The call may have ended, or they been kept out, meanwhile.
    if (_closed || peers[peer.id] != peer || peer._keptOut) {
      await connection.close();
      return;
    }
    peer._soundOut = connection;
    connection.onConnectionState = (state) {
      final to = 'screen sound to ${peer.id.substring(0, 8)}';
      _trace('$to: $state');
      if (state == RTCPeerConnectionState.RTCPeerConnectionStateConnected) {
        _traceSound(connection, to);
      }
      if (state != RTCPeerConnectionState.RTCPeerConnectionStateFailed ||
          peer._soundOut != connection) {
        return;
      }
      // Made again a moment later, if it is still wanted.
      peer._soundOut = null;
      connection.close();
      Future<void>.delayed(const Duration(seconds: 3), () => _soundTo(peer));
    };
    final offer = await connection.createOffer({});
    // Only Opus, as on the call's connections ([onlyCallCodecs]).
    final sdp = await _withCandidates(
      connection,
      RTCSessionDescription(onlyCallCodecs(offer.sdp ?? ''), 'offer'),
    );
    if (peer._soundOut == connection) {
      final message = {'t': 'sound', 'sdp': sdp};
      _signal(peer.id, message);
      _watchSound(peer, connection, message, 0);
    }
  }

  /// Sends the screen's sound offer [message] to [peer] again while [connection] has no
  /// answer: a signal lost on its way would leave [peer] without the screen's sound for as
  /// long as it watches.
  void _watchSound(
    CallPeer peer,
    RTCPeerConnection connection,
    Map<String, Object?> message,
    int tries,
  ) {
    if (tries >= Call._renegotiationTries) return;
    Future<void>.delayed(Call.renegotiationWait(tries), () async {
      if (_closed || peers[peer.id] != peer || peer._soundOut != connection) {
        return;
      }
      if (await connection.getRemoteDescription() != null) return;
      if (_closed || peer._soundOut != connection) return;
      _trace('no answer for the screen sound from ${peer.id.substring(0, 8)}');
      _signal(peer.id, message);
      _watchSound(peer, connection, message, tries + 1);
    });
  }

  /// Takes the sound of [peer]'s shared screen on the connection it offers for it: sound
  /// only, in the codecs calls keep.
  Future<void> _soundFrom(CallPeer peer, String sdp) async {
    if (!soundDescription(sdp)) {
      _trace(
        'screen sound from ${peer.id.substring(0, 8)} not one sound channel '
        'in Opus: refused',
      );
      return;
    }
    if (watching != peer.id || peer._keptOut) {
      _trace(
        'screen sound from ${peer.id.substring(0, 8)}, not watched: refused',
      );
      return;
    }
    await peer._closeSoundIn();
    final connection = await createPeerConnection(_network(peer.id));
    if (_closed || peers[peer.id] != peer || peer._keptOut) {
      await connection.close();
      return;
    }
    peer._soundIn = connection;
    connection.onTrack = (event) {
      if (event.track.kind == 'audio') {
        _setGain(event.track, screenSoundGain(peer.id));
      }
    };
    connection.onConnectionState = (state) {
      final from = 'screen sound from ${peer.id.substring(0, 8)}';
      _trace('$from: $state');
      if (state == RTCPeerConnectionState.RTCPeerConnectionStateConnected) {
        _traceSound(connection, from);
      }
    };
    await connection.setRemoteDescription(RTCSessionDescription(sdp, 'offer'));
    final answer = await connection.createAnswer({});
    final mine = await _withCandidates(
      connection,
      RTCSessionDescription(onlyCallCodecs(answer.sdp ?? ''), 'answer'),
    );
    if (peer._soundIn == connection) {
      _signal(peer.id, {'t': 'sound-answer', 'sdp': mine});
    }
  }

  /// With the call's log on, what a screen's sound connection carries 5 s after it opens.
  void _traceSound(RTCPeerConnection connection, String what) {
    if (!_verbose) return;
    Future<void>.delayed(const Duration(seconds: 5), () async {
      try {
        for (final report in await connection.getStats()) {
          final v = report.values;
          if (v['kind'] != 'audio') continue;
          if (report.type == 'outbound-rtp') {
            _trace('$what: ${v['packetsSent']} packets sent');
          } else if (report.type == 'inbound-rtp') {
            _trace(
              '$what: ${v['packetsReceived']} packets, level ${v['audioLevel']}',
            );
          } else if (report.type == 'media-source') {
            _trace('$what: level ${v['audioLevel']} at the source');
          }
        }
      } catch (e) {
        _trace('$what: $e');
      }
    });
  }
}
