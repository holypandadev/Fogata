part of '../app_state.dart';

/// How the window goes into a call and out of it: the room's call started and ended as
/// this device comes and goes, the call's page, and what the core tells of direct calls.
extension _Calls on AppState {
  /// A friend rung into the call did not come in: why, in a few words.
  Future<void> _onAddEnded(UiEvent e) async {
    final group = e.group!;
    final person =
        callState
            .callIn(group)
            ?.adding
            .where((p) => p.id == e.device)
            .firstOrNull ??
        contacts.friendPerson(e.device!);
    await refreshCalls();
    if (person != null) say(CallEnding(group, person, e.callEnd!).title);
  }

  void _leaveCallPage() {
    final group = roomState.roomGroup;
    roomState.memories.remove(group);
    roomState.outgoing.remove(group);
    roomState.dropPreviews();
    _dropVideos();
    roomState.roomGroup = null;
    roomState.room = null;
    callState.callEnding = null;
    _goBack();
    notifyListeners();
    refreshGroups();
    refreshCalls();
  }

  /// Shows a call's room: the core is in it already.
  Future<void> _enterCall(String group, {required bool answered}) async {
    final before = roomState.roomGroup;
    if (before != null && before != group) {
      roomState.memories.remove(before);
      roomState.outgoing.remove(before);
      roomState.dropPreviews();
    }
    _noteWayBack();
    roomState.roomGroup = group;
    roomState.roomVoice = AppState.firstRoom;
    nav.selectedGroup = null;
    roomState.room = null;
    roomState.roomTab = 0;
    // Settings that are open stay open, with what was being written in them: the call goes
    // on behind them, as a room does, its shown screen or camera floating over them.
    nav.userPanelFrom = null;
    nav.page = Screen.room;
    // The caller's room is new: the friend coming in is the first thing to hear. The one
    // answering hears its own way in, not the caller already there.
    if (answered) {
      _quietUntil[group] = DateTime.now().millisecondsSinceEpoch + 3000;
    }
    notifyListeners();
    await refreshGroups();
    await refreshRoom();
    final memory = roomState.memoryFor(
      group,
      roomState.room?.startedMs ?? DateTime.now().millisecondsSinceEpoch,
    );
    memory.joinedMs = DateTime.now().millisecondsSinceEpoch;
    memory.othersExpected = answered;
    await _startCall(group);
  }

  /// The friend answered, or this device's call and the friend's crossed and the core
  /// answered the friend's: that call is the one shown now.
  Future<void> _onCallAccepted(String group) async {
    await refreshCalls();
    if (roomState.roomGroup == group ||
        callState.callIn(group)?.phase != UiCallPhase.connected) {
      return;
    }
    _play(UiSound.join);
    await _enterCall(group, answered: true);
  }

  Future<void> _onCallEnded(UiEvent e) async {
    final group = e.group!;
    final reason = e.callEnd!;
    final device = e.device!;
    final person =
        callState.callIn(group)?.with_ ?? contacts.friendPerson(device);
    await refreshCalls();
    if (reason == UiCallEnd.missed ||
        reason == UiCallEnd.missedInCall ||
        reason == UiCallEnd.missedOffline) {
      notices.missedCalls[device] = DateTime.now().millisecondsSinceEpoch;
      if (reason == UiCallEnd.missedInCall) {
        notices.missedInCall.add(device);
      } else {
        notices.missedInCall.remove(device);
      }
      if (reason == UiCallEnd.missedOffline) {
        notices.missedOffline.add(device);
      } else {
        notices.missedOffline.remove(device);
      }
      notices.missedUnseen = nav.page != Screen.friends;
      if (notices.missedUnseen) {
        notices.noteMissed(device);
      } else {
        notices.missedNote = null;
      }
      notices.soundForNew();
      notifyListeners();
    }
    if (roomState.roomGroup != group) return;
    // Also while another page is shown: the call's sound and pictures stop.
    if (call?.group == group) await _endCall();
    // Stopping takes a moment, in which another room may have taken the window: it stays.
    if (roomState.roomGroup != group) return;
    switch (reason) {
      case UiCallEnd.noAnswer ||
          UiCallEnd.declined ||
          UiCallEnd.busy ||
          UiCallEnd.unreachable ||
          UiCallEnd.outdated:
        _play(UiSound.callEnded);
        final ending = person == null
            ? null
            : CallEnding(group, person, reason);
        if (ending != null && nav.page == Screen.room) {
          callState.callEnding = ending;
          notifyListeners();
        } else {
          if (ending != null) say(ending.title);
          _leaveCallPage();
        }
      case UiCallEnd.hungUp:
        _play(UiSound.callEnded);
        say(l10n.appHungUp(person?.name ?? l10n.appYourFriend));
        _leaveCallPage();
      case UiCallEnd.alone:
        _play(UiSound.callEnded);
        say(l10n.appCallEndedAlone);
        _leaveCallPage();
      // Answered here a moment after another computer open with this identity did.
      case UiCallEnd.elsewhere:
        _play(UiSound.callEnded);
        say(l10n.appAnsweredElsewhere);
        _leaveCallPage();
      case UiCallEnd.cancelled ||
          UiCallEnd.ended ||
          UiCallEnd.missed ||
          UiCallEnd.missedInCall ||
          UiCallEnd.missedOffline:
        _leaveCallPage();
    }
  }

  /// Call signals from a participant were dropped on their way here (the screens fell
  /// far behind): what the two agreed on may be half done on either side, so the
  /// connection with it is made anew, as with one that just came in.
  Future<void> _onSignalsLost(UiEvent e) async {
    final call = this.call;
    final from = e.device;
    if (call == null || call.group != e.group || from == null) return;
    final before = call.peers.keys.toList();
    if (!before.contains(from)) return;
    await call.setParticipants(before.where((id) => id != from).toList());
    if (!identical(this.call, call)) return;
    // Back with whoever is in the room now; with whoever was, when its room is not known
    // here (a direct call's, not shown).
    await call.setParticipants(_inCall(call) ?? before);
  }

  /// Who is in [call]'s room now as this device knows it, or null when it does not know it
  /// or is not in it.
  Iterable<String>? _inCall(Call call) {
    if (callState.callIn(call.group) != null) {
      final r = call.group == roomState.roomGroup ? roomState.room : null;
      if (r == null || !r.joined) return null;
      return r.participants.map((p) => p.person.id);
    }
    final summary = contacts.group(call.group)?.room;
    if (summary == null || !summary.joined) return null;
    return summary.participants.map((p) => p.id);
  }

  /// Keeps the call's connections in step with the room's participants.
  void _followCall() {
    final call = this.call;
    if (call == null) return;
    // A direct call's room is not among the groups: the room shown says who is in it.
    if (callState.callIn(call.group) != null) {
      final r = call.group == roomState.roomGroup ? roomState.room : null;
      if (r == null) return;
      if (!r.joined) {
        _endCall();
        return;
      }
      call.setParticipants(r.participants.map((p) => p.person.id));
      return;
    }
    final summary = contacts.group(call.group)?.room;
    if (summary == null || !summary.joined) {
      _endCall();
      return;
    }
    call.setParticipants(summary.participants.map((p) => p.id));
  }

  Future<void> _startCall(String group) async {
    final me = contacts.profile?.person.id;
    if (me == null) return;
    if (call?.group == group) return;
    // The settings' microphone test gives the microphone up first, and starts no more.
    final stopping = micTest.block();
    await _endCall(starting: true);
    await stopping;
    // Without a sound server WebRTC's voice engine would end the application (Linux): no
    // call is made then, and the room's chat goes on without it.
    final allowed = await soundServer();
    // The room may have been left or changed while the answer came (up to 10 s).
    if (roomState.roomGroup != group || call != null) {
      if (call == null) micTest.unblock();
      return;
    }
    if (!allowed) {
      micTest.unblock();
      say(l10n.callNoSoundSystem, error: true);
      // The others in the room would otherwise keep trying to connect to this device.
      _noCallIn = group;
      _noCallTold.clear();
      for (final p in roomState.room?.participants ?? const <UiParticipant>[]) {
        if (!p.me) _tellNoCall(group, p.person.id);
      }
      notifyListeners();
      return;
    }
    _noCallIn = null;
    final fresh = Call(
      group: group,
      me: me,
      mic: prefs.joinWithMic,
      camera: callState.joinWithCamera,
      input: prefs.audioInput,
      output: prefs.audioOutput,
      cameraId: prefs.videoInput,
      noiseSuppression: prefs.noiseSuppression,
      gainOf: gainOf,
      screenGainOf: screenGainOf,
      screenQuality: prefs.screenQuality,
      nameOf: (id) => nameOf(id, l10n.appSomeone),
    );
    // The chat shown by hand while the room opened stays shown for its call.
    if (roomState.chatSmallFor == group) roomState.chatSmallFor = fresh;
    // What the call tells (who connects, mutes, shares) goes to what shows the call only.
    callState.call = fresh;
    callState.joinWithCamera = false;
    fresh.onSound = _play;
    fresh.onProblem = (text) => say(text, error: true);
    fresh.onNotice = say;
    fresh.onMicUnusable = (text) => micUnusable(fresh, text);
    await fresh.start();
    _followCall();
  }

  void _tellNoCall(String group, String device) {
    final now = DateTime.now();
    final last = _noCallTold[device];
    if (last != null && now.difference(last) < const Duration(seconds: 10)) {
      return;
    }
    _noCallTold[device] = now;
    sendSignal(group, device, jsonEncode({'t': Call.noVoice}));
  }

  /// Ends the call. The microphone test may start again after, unless another call is
  /// [starting].
  Future<void> _endCall({bool starting = false}) async {
    // Out of the room, or into another call: nobody to tell there is none.
    _noCallIn = null;
    final ending = call;
    callState.call = null;
    if (ending != null) {
      await ending.stop();
      notifyListeners();
    }
    if (!starting && call == null) micTest.unblock();
  }
}
