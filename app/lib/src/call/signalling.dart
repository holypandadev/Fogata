part of '../call.dart';

/// Signals from devices that have no connection here yet. The room's participants and the
/// signals from them arrive by different ways and in either order: a signal that comes
/// first waits here until its sender is a participant and the call has made its connection
/// with it (a call that is starting makes none before its media bridge and microphone are
/// ready), and signals from anyone else are dropped, so a member of the group that is not
/// in the room cannot open a connection and receive what this device sends.
@visibleForTesting
class EarlySignals {
  /// Signals kept per device; older ones are dropped.
  static const perDevice = 4;

  final Set<String> _participants = {};
  final Map<String, List<Map<String, dynamic>>> _waiting = {};

  /// The participants now; signals waiting from devices no longer among them are dropped.
  void participants(Iterable<String> ids) {
    _participants
      ..clear()
      ..addAll(ids);
    _waiting.removeWhere((id, _) => !_participants.contains(id));
  }

  bool isParticipant(String id) => _participants.contains(id);

  /// Keeps a signal from a device that is not a participant yet.
  void keep(String from, Map<String, dynamic> message) {
    final list = _waiting.putIfAbsent(from, () => []);
    list.add(message);
    if (list.length > perDevice) list.removeAt(0);
  }

  /// The signals waiting from `from`, in order; they are forgotten.
  List<Map<String, dynamic>> take(String from) =>
      _waiting.remove(from) ?? const [];

  int get waiting => _waiting.values.fold(0, (n, l) => n + l.length);
}

/// The first wait before a renegotiation that had no answer goes again (see
/// [Call.renegotiationWait]); changed by tests.
@visibleForTesting
Duration renegotiationAfter = const Duration(seconds: 10);

/// How often a call tells its state (microphone, camera, screen, whose screen it watches)
/// again, so that one lost on its way is not lost for good; changed by tests.
@visibleForTesting
Duration stateAgainEvery = const Duration(seconds: 15);

/// Where a call's signals go instead of the core, for tests.
@visibleForTesting
void Function(String to, Map<String, dynamic> message)? signalsForTests;

/// How a call agrees with each participant on a connection: the signals it sends and
/// takes, its offers and answers, and the tries that keep them going until one is agreed.
extension _Signalling on Call {
  /// Offers again, or asks for an offer, until the participant answers. A signal to a
  /// member this device is not connected to yet is dropped on the way, and members connect
  /// to each other at different moments.
  void _watch(CallPeer peer) {
    // One loop at a time for each participant: a new one ends any before it.
    final round = ++peer._watching;
    final wait = Duration(seconds: 5 + 5 * peer._waits.clamp(0, 5));
    Future<void>.delayed(wait, () async {
      if (_closed ||
          peers[peer.id] != peer ||
          peer._watching != round ||
          peer._described ||
          peer._keptOut ||
          peer.voiceless) {
        return;
      }
      peer._waits++;
      _trace('nothing yet from ${peer.id.substring(0, 8)}; trying again');
      try {
        if (me.compareTo(peer.id) < 0) {
          await peer._inTurn(() => _offer(peer));
        } else {
          _signal(peer.id, {'t': 'again'});
        }
      } catch (e) {
        _trace('trying again: $e');
      }
      // A newer round started while this one offered: that one goes on, not this.
      if (peer._watching != round) return;
      _watch(peer);
    });
  }

  /// Waits until [CallPeer.offerEvery] has passed since an offer of [peer] was last taken
  /// ([taken]).
  Future<void> _offerTurn(CallPeer peer, DateTime taken) async {
    final wait = CallPeer.offerEvery - DateTime.now().difference(taken);
    if (wait > Duration.zero) await Future<void>.delayed(wait);
  }

  Future<void> _handleSafely(
    CallPeer peer,
    String from,
    Map<String, dynamic> message,
  ) async {
    try {
      await _handle(peer, from, message);
    } catch (e) {
      // For example an answer to an offer that was replaced meanwhile.
      _trace('${message['t']} from ${from.substring(0, 8)}: $e');
    }
  }

  Future<void> _handle(
    CallPeer peer,
    String from,
    Map<String, dynamic> message,
  ) async {
    switch (message['t']) {
      case 'offer':
        // Only the device with the smaller id offers (see [_offer] and [_renegotiate]),
        // and an offer has the call's channels and no others.
        final sdp = message['sdp'];
        if (me.compareTo(from) < 0 ||
            sdp is! String ||
            !callSections(sdp) ||
            !callCodecs(sdp)) {
          _trace('offer from ${from.substring(0, 8)} not fit: dropped');
          return;
        }
        final connection = peer.connection;
        if (message['update'] == true &&
            connection != null &&
            peer._described) {
          await connection.setRemoteDescription(
            RTCSessionDescription(peerBitrates(sdp), 'offer'),
          );
          await _reply(peer, connection, message['n']);
        } else {
          await _answer(peer, message['sdp'] as String, message['n']);
        }
      case 'answer':
        final connection = peer.connection;
        if (connection == null) return;
        final number = message['n'];
        if (number is int && number != peer._offerNumber) {
          _trace(
            'answer to an earlier offer from ${from.substring(0, 8)}: dropped',
          );
          return;
        }
        // What this device decodes is what its own offer listed, but an answer in other
        // codecs is not one a copy sends either.
        final sdp = message['sdp'];
        if (sdp is! String || !callCodecs(sdp)) {
          _trace('answer from ${from.substring(0, 8)} not fit: dropped');
          return;
        }
        await connection.setRemoteDescription(
          RTCSessionDescription(peerBitrates(sdp), 'answer'),
        );
        peer._offering = false;
        await _described(peer);
        if (peer._again) {
          peer._again = false;
          await _renegotiate(peer);
        } else {
          peer._agreed?.complete();
          peer._agreed = null;
        }
      case 'update':
        // The other side changed what it sends and asks for a new offer.
        await _renegotiate(peer);
      case 'ice':
        final candidate = RTCIceCandidate(
          message['candidate'] as String?,
          message['mid'] as String?,
          message['index'] as int?,
        );
        if (peer._described && peer.connection != null) {
          if (peer._candidates++ >= CallPeer.maxCandidates) return;
          await peer.connection!.addCandidate(candidate);
        } else if (peer._early.length < CallPeer.earlyCandidates) {
          // A few, as a connection has: someone who never answers cannot fill the memory.
          peer._early.add(candidate);
        }
      case 'state':
        final shared = peer.cam || peer.sharing;
        peer.mic = message['mic'] == true;
        peer.cam = message['cam'] == true;
        peer.sharing = message['screen'] == true;
        peer.sound = message['sound'] == true;
        final shares = peer.cam || peer.sharing;
        if (shares != shared) {
          onSound?.call(shares ? UiSound.shareStart : UiSound.shareStop);
        }
        if (!peer.sharing && watching == peer.id) watching = null;
        if (!peer.cam && focused == peer.id) focused = null;
        _showBack(peer);
        final watch = message['watch'];
        final watched = peer.watches;
        peer.watches = watch is String && watch.isNotEmpty ? watch : null;
        if (sharing && (watched == me) != (peer.watches == me)) {
          // Someone starts or stops watching this device's screen.
          onSound?.call(
            peer.watches == me ? UiSound.watchStart : UiSound.watchStop,
          );
          _watchersChanged(peer);
          await _resendTo(peer, _screen);
          await _resendTo(peer, _screenSound);
          // The screen goes to them as it goes to everyone now: their connection was last
          // told how when it was made, or when they last watched, and the size and rate
          // may have changed since (more watching, or the processor).
          final connection = peer.connection;
          if (peer.watches == me && connection != null) {
            await _tune(peer, connection);
            // Its capture's last picture again, for their encoder: a screen that stays
            // still gives it no new one (see [_refreshScreen]).
            await _refreshScreen();
          }
          await _soundTo(peer);
        }
      case 'sound':
        // The sound of their shared screen, offered on a connection of its own.
        await _soundFrom(peer, message['sdp'] as String);
      case 'sound-answer':
        final sdp = message['sdp'];
        if (sdp is! String || !soundDescription(sdp)) {
          _trace(
            'screen sound answer from ${from.substring(0, 8)} not fit: dropped',
          );
          return;
        }
        await peer._soundOut?.setRemoteDescription(
          RTCSessionDescription(peerBitrates(sdp), 'answer'),
        );
      case 'sound-end':
        await peer._closeSoundIn();
      case 'again':
        // The other side has no working connection and asks for a new offer; one made a
        // moment ago is still on its way, and another would only replace it (both sides
        // ask and offer again after a failure, and every new offer undid the last).
        final made = peer._madeAt;
        final fresh =
            made != null &&
            DateTime.now().difference(made) < Call.connectWithin &&
            peer.state != RTCPeerConnectionState.RTCPeerConnectionStateFailed;
        if (me.compareTo(from) < 0 && !peer.connected && !fresh) {
          await _offer(peer);
        }
    }
  }

  void _signal(String to, Map<String, dynamic> message) {
    if (signalsForTests case final sent?) {
      sent(to, message);
      return;
    }
    // Nobody to tell before the call started (a room drawn without one, as in tests).
    if (_bridge == null) return;
    _trace('send ${message['t']} to ${to.substring(0, 8)}');
    api
        .sendSignal(groupId: group, deviceId: to, payload: jsonEncode(message))
        .catchError((_) {});
  }

  Map<String, dynamic> get _state => {
    't': 'state',
    'mic': mic,
    'cam': camera,
    'screen': sharing,
    'sound': sharingSound,
    'watch': watching ?? '',
  };

  void _sendState() {
    for (final id in peers.keys) {
      _signal(id, _state);
    }
  }

  /// Every connection reaches only this device's media bridge, as one for [member]: the
  /// bridge sends what it relays to that member alone, and hands it only what that member
  /// sends (see `crates/fogata-media/src/bridge.rs`).
  Map<String, dynamic> _network(String member) {
    final bridge = _bridge!;
    return {
      'iceServers': [
        {
          'urls': 'turn:${bridge.host}:${bridge.port}?transport=udp',
          'username': '${bridge.username}.$member',
          'credential': bridge.password,
        },
      ],
      'iceTransportPolicy': 'relay',
      'bundlePolicy': 'max-bundle',
      'rtcpMuxPolicy': 'require',
      'sdpSemantics': 'unified-plan',
    };
  }

  /// Tries again a few seconds after a connection failed, while both are in the room.
  void _retry(CallPeer peer) {
    final attempt = ++peer._attempt;
    final failed = peer.connection;
    Future<void>.delayed(
      // Short pauses: a new connection that does not come up is given up on after
      // [connectWithin] anyway, and signals sent while the member is away go nowhere.
      Duration(seconds: 2 + 2 * attempt.clamp(0, 2)),
      () async {
        // Done already: connected again, or another connection made meanwhile (the other
        // side offered, or another try).
        if (_closed ||
            peers[peer.id] != peer ||
            peer.connected ||
            peer._keptOut ||
            peer.connection != failed) {
          return;
        }
        try {
          if (me.compareTo(peer.id) < 0) {
            await peer._inTurn(() => _offer(peer));
          } else {
            _signal(peer.id, {'t': 'again'});
          }
        } catch (e) {
          _trace('trying again after a failure: $e');
        }
      },
    );
  }

  Future<void> _offer(CallPeer peer) async {
    final connection = await _connect(peer);
    peer._offering = true;
    final number = ++peer._offerNumber;
    for (final channel in [_audio, _camera, _screen, _screenSound]) {
      await connection.addTransceiver(
        kind: channel == _audio || channel == _screenSound
            ? RTCRtpMediaType.RTCRtpMediaTypeAudio
            : RTCRtpMediaType.RTCRtpMediaTypeVideo,
        init: RTCRtpTransceiverInit(
          direction: switch (channel) {
            _audio => _voiceDirection(peer),
            _screenSound => _soundDirection,
            _ => TransceiverDirection.SendRecv,
          },
        ),
      );
    }
    final transceivers = await _channels(connection);
    for (var i = 0; i < transceivers.length; i++) {
      await transceivers[i].sender.replaceTrack(_track(i, peer));
    }
    final offer = await connection.createOffer({});
    final sdp = await _describe(peer, connection, offer, 'offer');
    await _tune(peer, connection);
    _signal(peer.id, {'t': 'offer', 'sdp': sdp, 'n': number});
  }

  Future<void> _answer(CallPeer peer, String sdp, Object? number) async {
    final connection = await _connect(peer);
    await connection.setRemoteDescription(
      RTCSessionDescription(peerBitrates(sdp), 'offer'),
    );
    await _reply(peer, connection, number);
    await _described(peer);
  }

  /// Answers the offer just set on [connection], sending what is on here.
  /// [number] is the offer's, sent back with the answer (copies before it send none).
  Future<void> _reply(
    CallPeer peer,
    RTCPeerConnection connection,
    Object? number,
  ) async {
    final transceivers = await _channels(connection);
    for (var i = 0; i < transceivers.length; i++) {
      await transceivers[i].setDirection(switch (i) {
        _audio => _voiceDirection(peer),
        _camera || _screen => TransceiverDirection.SendRecv,
        _screenSound => _soundDirection,
        // A call has no other channel: nothing comes on one (see [callSections]).
        _ => TransceiverDirection.Inactive,
      });
      await transceivers[i].sender.replaceTrack(_track(i, peer));
    }
    final answer = await connection.createAnswer({});
    final sdp = await _describe(peer, connection, answer, 'answer');
    await _tune(peer, connection);
    _signal(peer.id, {'t': 'answer', 'sdp': sdp, 'n': ?number});
    peer._agreed?.complete();
    peer._agreed = null;
  }

  /// Sets this side's description and returns it with its network candidates inside, once
  /// gathered, which is quick: the only server is this device's bridge. A description that
  /// arrives brings its candidates, so none can be lost on the way or reach a later
  /// connection. Candidates sent apart (by older versions) are still taken.
  Future<String> _describe(
    CallPeer peer,
    RTCPeerConnection connection,
    RTCSessionDescription description,
    String type,
  ) async {
    final sdp = screenSoundStereo(
      voiceDtx(
        cameraBitrate(
          screenBitrate(
            voiceRedFirst(
              onlyCallCodecs(vp8First(description.sdp ?? ''), voiceRed: true),
            ),
          ),
        ),
      ),
      _screenSound,
    );
    await connection.setLocalDescription(RTCSessionDescription(sdp, type));
    await peer._gathered.future.timeout(
      const Duration(seconds: 3),
      onTimeout: () => _trace('candidates still coming for ${peer.id}'),
    );
    return (await connection.getLocalDescription())?.sdp ?? sdp;
  }

  /// Agrees again on what each side sends, on the connection there is. Only the side that
  /// made the first offer offers again, so two offers never cross; the other side asks it.
  Future<void> _renegotiate(CallPeer peer) async {
    if (me.compareTo(peer.id) > 0) {
      _signal(peer.id, {'t': 'update'});
      _watchUpdate(peer, peer._offersIn, 0);
      return;
    }
    final connection = peer.connection;
    if (connection == null) return;
    if (peer._offering || !peer._described) {
      peer._again = true;
      return;
    }
    peer._offering = true;
    final number = ++peer._offerNumber;
    final String sdp;
    try {
      final offer = await connection.createOffer({});
      sdp = await _describe(peer, connection, offer, 'offer');
    } catch (_) {
      // Left offering, every later renegotiation would wait for this one forever.
      if (peer.connection == connection) peer._offering = false;
      rethrow;
    }
    final message = {'t': 'offer', 'sdp': sdp, 'update': true, 'n': number};
    _signal(peer.id, message);
    _watchOffer(peer, number, message, 0);
  }

  /// Sends offer [message] (number [number]) again while it is still the one waiting for
  /// its answer: a signal lost on its way (one past a member's rate, a link down a moment)
  /// would otherwise leave every later renegotiation waiting behind it for the rest of the
  /// connection, and a camera or a screen turned on never reach the other side.
  void _watchOffer(
    CallPeer peer,
    int number,
    Map<String, Object?> message,
    int tries,
  ) {
    Future<void>.delayed(Call.renegotiationWait(tries), () {
      if (_closed ||
          peers[peer.id] != peer ||
          !peer._offering ||
          peer._offerNumber != number) {
        return;
      }
      if (tries >= Call._renegotiationTries) {
        // Given up on: a later change offers anew rather than waiting behind it for the
        // rest of the connection, and one that waited meanwhile goes now. A late answer
        // to this offer is told apart by its number.
        _trace('no answer from ${peer.id.substring(0, 8)}; offering anew');
        peer._offering = false;
        if (peer._again) {
          peer._again = false;
          unawaited(_renegotiate(peer));
        }
        return;
      }
      _trace('no answer from ${peer.id.substring(0, 8)}; offering again');
      _signal(peer.id, message);
      _watchOffer(peer, number, message, tries + 1);
    });
  }

  /// Asks for a renegotiation again while no offer came since [offersBefore]: the side that
  /// offers never heard the asking.
  void _watchUpdate(CallPeer peer, int offersBefore, int tries) {
    if (tries >= Call._renegotiationTries) return;
    Future<void>.delayed(Call.renegotiationWait(tries), () {
      if (_closed || peers[peer.id] != peer || peer._offersIn != offersBefore) {
        return;
      }
      _trace('no offer from ${peer.id.substring(0, 8)}; asking again');
      _signal(peer.id, {'t': 'update'});
      _watchUpdate(peer, offersBefore, tries + 1);
    });
  }

  /// Sets this side's description of a screen's sound connection and returns it with its
  /// network candidates inside, as [_describe] does for the call's connections.
  Future<String> _withCandidates(
    RTCPeerConnection connection,
    RTCSessionDescription description,
  ) async {
    final gathered = Completer<void>();
    connection.onIceGatheringState = (state) {
      if (state == RTCIceGatheringState.RTCIceGatheringStateComplete &&
          !gathered.isCompleted) {
        gathered.complete();
      }
    };
    await connection.setLocalDescription(description);
    await gathered.future.timeout(const Duration(seconds: 3), onTimeout: () {});
    return (await connection.getLocalDescription())?.sdp ??
        description.sdp ??
        '';
  }

  Future<void> _described(CallPeer peer) async {
    peer._described = true;
    for (final candidate in peer._early) {
      await peer.connection?.addCandidate(candidate);
    }
    peer._early.clear();
  }

  /// The three channels of a connection, in order.
  Future<List<RTCRtpTransceiver>> _channels(
    RTCPeerConnection? connection,
  ) async {
    if (connection == null) return const [];
    final all = await connection.getTransceivers();
    // Before the first description the channels have no mid yet and keep their order.
    if (all.every((t) => int.tryParse(t.mid) != null)) {
      all.sort((a, b) => int.parse(a.mid).compareTo(int.parse(b.mid)));
    }
    return all;
  }
}
