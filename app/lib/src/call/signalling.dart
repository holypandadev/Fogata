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

  /// Forgets the signals waiting from `from`.
  void forget(String from) => _waiting.remove(from);

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

/// The same, with the sender, for tests of several calls that signal each other.
@visibleForTesting
void Function(String from, String to, Map<String, dynamic> message)?
signalsBetweenForTests;

/// How a call agrees with each participant on a connection: the signals it sends and
/// takes, its offers and answers, and the tries that keep them going until one is agreed.
extension _Signalling on Call {
  /// Offers again, or asks for an offer, until the participant answers. A signal to a
  /// member this device is not connected to yet is dropped on the way, and members connect
  /// to each other at different moments.
  void _watch(CallPeer peer) {
    // One loop at a time for each participant: a new one ends any before it.
    final round = ++peer._watching;
    // Let go of already: nothing would cancel the wait ([CallPeer.dispose]), and once due
    // it would do nothing.
    if (_closed || peer._gone) return;
    final wait = Duration(seconds: 5 + 5 * peer._waits.clamp(0, 5));
    // A round before this one would do nothing once due.
    peer._watchWait?.cancel();
    peer._watchWait = Timer(wait, () async {
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
          await peer._inTurn(() async {
            // An offer still waiting for its answer goes again as it is: it may only be
            // slow on its way, or its answer on theirs, and a new one would throw that
            // negotiation away; a copy of a lost one does what a new one would. One that
            // cannot be agreed any more is made anew, and so is one to a copy whose
            // signals say nothing of answering a copy with the answer it gave (copies
            // before it make their end anew, and the offer's first answer would be set on
            // this one). With nothing heard from them yet it goes as a copy: the offer
            // most likely went before they could take it, or is slow on its way.
            final out = peer._offerOut;
            final made = peer._madeAt;
            if (peer._answersWhich != false &&
                out != null &&
                out.on == peer.connection &&
                peer._offering &&
                !peer._described &&
                !peer._blind &&
                peer.state !=
                    RTCPeerConnectionState.RTCPeerConnectionStateFailed &&
                made != null &&
                callClock().difference(made) < Call.connectWithin) {
              _trace('offer to ${_short(peer.id)} sent again');
              _signal(peer.id, out.message);
              return;
            }
            await _offer(peer);
          });
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
    final wait = CallPeer.offerEvery - callClock().difference(taken);
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
    if (!await _theirCall(peer, from, message)) return;
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
        } else if (_answeredAlready(peer, message)) {
          _trace(
            'offer ${message['n']} from ${_short(from)} answered already: '
            'its answer sent again',
          );
          _signal(from, peer._answered!.answer);
        } else {
          await _answer(peer, message);
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
        // A change waiting for this answer goes with the offer [_voiceTo] makes, if it
        // makes one: an offer has everything asked so far.
        final again = peer._again;
        peer._again = false;
        final more = await _voiceTo(peer);
        if (again && !more) {
          await _renegotiate(peer);
        } else if (!more) {
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
        // Their states in the order they told them: each signal goes on its own, and an
        // older one that arrives late would undo a newer (a screen shown as off, and no
        // longer watched here). One that names no number (from a copy before it) is taken.
        // A state names the call of theirs it is from (`sc`): one from another call than
        // the one their offers, answers and askings named is a late one of a call that
        // ended, or one of their next call ahead of its offer (whose connection, once up,
        // brings its state again). Numbers go on only within one call of theirs.
        final call = message['sc'] is String ? message['sc'] as String : null;
        final known = peer._theirCall;
        if (call != null && known != null && call != known) {
          _trace('state from another call of ${_short(from)}: dropped');
          return;
        }
        final number = message['s'];
        if (number is int) {
          final last = peer._stateIn;
          if (last != null && last.call == call && number < last.number) {
            _trace(
              'state $number from ${_short(from)} older than ${last.number}: '
              'dropped',
            );
            return;
          }
          peer._stateIn = (call: call, number: number);
        }
        final shared = peer.cam || peer.sharing;
        peer.mic = message['mic'] == true;
        peer.cam = message['cam'] == true;
        peer.sharing = message['screen'] == true;
        peer.sound = message['sound'] == true;
        final shares = peer.cam || peer.sharing;
        if (shares != shared) {
          onSound?.call(shares ? UiSound.shareStart : UiSound.shareStop);
        }
        if (!peer.sharing && watching == peer.id) {
          _watchingTo(null, 'its sharer\'s state says its screen is off');
        }
        if (!peer.cam && focused == peer.id) focused = null;
        _showBack(peer);
        final watch = message['watch'];
        final watched = peer.watches;
        _setWatches(peer, watch is String && watch.isNotEmpty ? watch : null);
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
      case 'which':
        // They ask which call this device has now (see [_askWhich]): the token goes back
        // with it, so they know the answer is not an older one.
        final token = message['k'];
        if (token is String) {
          _signal(from, {'t': 'this', 'k': token, 'now': _callId});
        }
      case 'this':
        final question = peer._asked;
        final now = message['now'];
        if (question == null ||
            message['k'] != question.token ||
            now is! String) {
          _trace(
            'answer on which call from ${_short(from)} not asked: dropped',
          );
          return;
        }
        await _theirCallNow(peer, from, question, now);
      case 'again':
        // The other side has no working connection and asks for a new offer; one made a
        // moment ago is still on its way, and another would only replace it (both sides
        // ask and offer again after a failure, and every new offer undid the last). One
        // made while the link to them was down went nowhere, however recent. Counted from
        // its answer, if one came: an answer slow on its way leaves it its whole time to
        // come up, as for [Call._connect]'s own wait.
        final made = peer._describedAt ?? peer._madeAt;
        final fresh =
            made != null &&
            !peer._blind &&
            callClock().difference(made) < Call.connectWithin &&
            peer.state != RTCPeerConnectionState.RTCPeerConnectionStateFailed;
        if (me.compareTo(from) < 0 && !peer.connected && !fresh) {
          await _offer(peer);
        }
    }
  }

  /// Follows which call of [from]'s device [message] comes from (its `c`, on what starts or
  /// agrees a connection): whether it is still taken. One from a call of theirs that ended
  /// is not. One from a call not seen before, when another was, says their call is a new
  /// one, which the room may never have shown (they left and entered again at once, or
  /// their application started again faster than WebRTC finds a connection dead): the
  /// connection with the one that ended is dropped at once, and the first connection is
  /// made as for someone who just came in, with no wait for that connection to be given
  /// up on. Only for a participant whose connection is here already: a call's id alone
  /// makes no connection with anyone.
  ///
  /// An offer or an asking of another call than the one known may also be a late one of an
  /// older call that this side does not remember as ended (this call started after it, or
  /// so many ended since): taken as theirs, it would end the call they have, and drop all
  /// its signals after. From a participant whose signals say it answers (`w`), such a
  /// signal is held, and they are asked which call is theirs now ([_askWhich]). An answer
  /// to this side's latest offer was made after that offer, by the call they had then, and
  /// is taken at once.
  Future<bool> _theirCall(
    CallPeer peer,
    String from,
    Map<String, dynamic> message,
  ) async {
    final theirs = message['c'];
    if (theirs is! String) return true;
    // A late signal of a call before an update says nothing of the copy they run now.
    if (message['w'] == 1) {
      peer._answersWhich = true;
    } else {
      peer._answersWhich ??= false;
    }
    if (peer._endedCalls.contains(theirs)) {
      _trace(
        '${message['t']} from an ended call of ${from.substring(0, 8)}: dropped',
      );
      return false;
    }
    // An answer to an earlier offer of this side says nothing of the call they have now (a
    // call of theirs that answered it late may have ended since); it is dropped below.
    final number = message['n'];
    if (message['t'] == 'answer' &&
        number is int &&
        number != peer._offerNumber) {
      return true;
    }
    final before = peer._theirCall;
    if (before != null &&
        before != theirs &&
        message['t'] != 'answer' &&
        peer._answersWhich == true) {
      _askWhich(peer, theirs, message);
      return false;
    }
    peer._theirCall = theirs;
    // The call they had when they last left the room, if this one is another: it ended.
    final left = peer._callWhenLeft;
    peer._callWhenLeft = null;
    if (before == null && left != null && left != theirs) peer._ended(left);
    if (before == null || before == theirs) return true;
    await _theirNewCall(peer, from, before, '${message['t']}');
    return true;
  }

  /// [peer]'s call is a new one, no longer [before], as a signal of type [type] showed:
  /// the connection with the one that ended is dropped at once and the first connection
  /// made anew.
  Future<void> _theirNewCall(
    CallPeer peer,
    String from,
    String before,
    String type,
  ) async {
    peer._ended(before);
    // A question asked before is answered by a call of theirs that may be older than this.
    peer._asked?.wait?.cancel();
    peer._asked = null;
    // Their new call numbers its states anew (see [Call._stateNumber]).
    peer._stateIn = null;
    _trace('${from.substring(0, 8)} has a new call; connecting anew');
    // An answer of their new call answers an offer of this side made since it started:
    // its connection is the right one.
    if (type == 'answer') return;
    await peer._close();
    peer._madeAt = null;
    peer._attempt = 0;
    peer._waits = 0;
    peer._blind = false;
    _watch(peer);
    // An offer is answered and an asking offered to as they are; anything else from the
    // new call starts the first connection now.
    if (type != 'offer' && type != 'again') {
      if (me.compareTo(from) < 0) {
        await _offer(peer);
      } else {
        _signal(from, {'t': 'again'});
      }
    }
    // What shows them connected follows once the signal is handled ([onSignal]).
  }

  /// Asks [peer] which call of theirs is the one now (`which`, with a token made for the
  /// question), holding [message], which named their call [theirs] and not the one known.
  /// A late signal cannot carry the token, and their answer (`this`) names the call they
  /// have once they read the question. One question at a time: a later signal of the same
  /// call is held in place of the one before, under the same question; one of another
  /// call gets a question of its own, as an answer to the one before may have been made
  /// before that call began. Asked every [Call.whichEvery], [Call.whichTries] times; with
  /// no answer meanwhile the call known is kept, and an answer that comes later is still
  /// taken (a new call of theirs that is real goes on offering or asking, which asks the
  /// same again).
  void _askWhich(CallPeer peer, String theirs, Map<String, dynamic> message) {
    final asked = peer._asked;
    if (asked != null && asked.call == theirs) {
      asked.message = message;
      if (asked.wait == null) {
        asked.tries = 0;
        _askWhichNow(peer, asked);
      }
      return;
    }
    asked?.wait?.cancel();
    final question = peer._asked = _Asked(Call._newCallId(), theirs, message);
    _trace(
      '${message['t']} from another call of ${_short(peer.id)}: '
      'asking which is theirs now',
    );
    _askWhichNow(peer, question);
  }

  void _askWhichNow(CallPeer peer, _Asked question) {
    question.tries++;
    _signal(peer.id, {'t': 'which', 'k': question.token});
    question.wait = Timer(Call.whichEvery, () {
      if (_closed || peers[peer.id] != peer || peer._asked != question) return;
      if (question.tries >= Call.whichTries) {
        _trace(
          'no answer yet from ${_short(peer.id)} on which call is theirs: '
          'the one known kept',
        );
        question.wait = null;
        return;
      }
      _askWhichNow(peer, question);
    });
  }

  /// Their answer [now] to the question [question]: the call they have. The signal held
  /// is taken when it is of that call; a call known before that is not it ended.
  Future<void> _theirCallNow(
    CallPeer peer,
    String from,
    _Asked question,
    String now,
  ) async {
    peer._asked = null;
    question.wait?.cancel();
    // Whatever this side took for ended, the answer is what they have.
    peer._endedCalls.remove(now);
    final known = peer._theirCall;
    if (question.call != now) peer._ended(question.call);
    if (now == known) {
      _trace(
        '${question.message['t']} from ${_short(from)} was of an older call: '
        'dropped',
      );
      return;
    }
    peer._theirCall = now;
    peer._callWhenLeft = null;
    final type = question.call == now ? '${question.message['t']}' : 'this';
    if (known != null) await _theirNewCall(peer, from, known, type);
    if (question.call == now) await _handle(peer, from, question.message);
  }

  void _signal(String to, Map<String, dynamic> message) {
    // What starts or agrees a connection says which call of this device it is from, and
    // that this device answers a `which` (see [_theirCall]); copies that do not know the
    // fields leave them.
    if (const {'offer', 'answer', 'again'}.contains(message['t'])) {
      message = {...message, 'c': _callId, 'w': 1};
    }
    if (signalsBetweenForTests case final sent?) {
      sent(me, to, message);
      return;
    }
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

  /// This device's state, numbered after every state it told before and naming this call
  /// (see 'state' in [_handle]); copies that do not know these fields leave them.
  Map<String, dynamic> get _state => {
    't': 'state',
    'sc': _callId,
    's': _stateNumber = math.max(
      _stateNumber + 1,
      DateTime.now().millisecondsSinceEpoch,
    ),
    'mic': mic,
    'cam': camera,
    'screen': sharing,
    'sound': sharingSound,
    'watch': watching ?? '',
  };

  void _sendState() {
    final state = _state;
    for (final id in peers.keys) {
      _signal(id, state);
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
    final voice = await _voiceWanted(peer, connection);
    for (final channel in [_audio, _camera, _screen, _screenSound]) {
      final direction = switch (channel) {
        _audio => voice,
        _screenSound => _soundDirection,
        _ => TransceiverDirection.SendRecv,
      };
      await connection.addTransceiver(
        kind: channel == _audio || channel == _screenSound
            ? RTCRtpMediaType.RTCRtpMediaTypeAudio
            : RTCRtpMediaType.RTCRtpMediaTypeVideo,
        init: RTCRtpTransceiverInit(direction: direction),
      );
      (_asked[connection] ??= {})[channel] = direction;
    }
    final transceivers = await _channels(connection);
    for (var i = 0; i < transceivers.length; i++) {
      await transceivers[i].sender.replaceTrack(_track(i, peer));
    }
    final offer = await connection.createOffer({});
    final sdp = await _describe(peer, connection, offer, 'offer');
    await _tune(peer, connection);
    final message = <String, dynamic>{'t': 'offer', 'sdp': sdp, 'n': number};
    if (peer.connection == connection) {
      peer._offerOut = (on: connection, message: message);
    }
    _signal(peer.id, message);
    _sentWhileGone(peer);
  }

  Future<void> _answer(CallPeer peer, Map<String, dynamic> offer) async {
    final connection = await _connect(peer);
    await connection.setRemoteDescription(
      RTCSessionDescription(peerBitrates(offer['sdp'] as String), 'offer'),
    );
    final number = offer['n'];
    final answer = await _reply(peer, connection, number);
    final theirs = offer['c'];
    if (theirs is String && number is int && peer.connection == connection) {
      peer._answered = (
        call: theirs,
        number: number,
        on: connection,
        answer: answer,
      );
    }
    await _described(peer);
  }

  /// [offer] is a copy of the first offer [peer]'s connection was made for (the same call
  /// of theirs, the same number), which a signal can be: answered again, it would make
  /// the connection anew and drop the one that works.
  bool _answeredAlready(CallPeer peer, Map<String, dynamic> offer) {
    final answered = peer._answered;
    return answered != null &&
        answered.on == peer.connection &&
        answered.call == offer['c'] &&
        answered.number == offer['n'];
  }

  /// Answers the offer just set on [connection], sending what is on here, and returns the
  /// answer sent. [number] is the offer's, sent back with the answer (copies before it
  /// send none).
  Future<Map<String, dynamic>> _reply(
    CallPeer peer,
    RTCPeerConnection connection,
    Object? number,
  ) async {
    final transceivers = await _channels(connection);
    final voice = await _voiceWanted(peer, connection);
    for (var i = 0; i < transceivers.length; i++) {
      await _ask(connection, transceivers, i, switch (i) {
        _audio => voice,
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
    final message = <String, dynamic>{'t': 'answer', 'sdp': sdp, 'n': ?number};
    _signal(peer.id, message);
    _sentWhileGone(peer);
    // The voice channel as the microphone asks, now that this is agreed: a send stream
    // made by it starts with its encoding on, or off as set while it had none.
    if (!await _voiceTo(peer)) {
      peer._agreed?.complete();
      peer._agreed = null;
    }
    return message;
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
    peer._describedAt ??= callClock();
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
