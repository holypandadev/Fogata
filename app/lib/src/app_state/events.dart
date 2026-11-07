part of '../app_state.dart';

/// What the window does with what the core tells: each event taken to its part of the
/// state, and the room's changes heard, said and shown as they come.
extension _Events on AppState {
  void _onTyping(UiEvent e) {
    final device = e.device;
    if (device == null || device == contacts.profile?.person.id) return;
    final chat = e.group == null
        ? TypingBoard.direct(device)
        : TypingBoard.room(e.group!, e.voice ?? AppState.firstRoom);
    if (e.kind == UiEventKind.typing) {
      typing.heard(chat, device, DateTime.now().millisecondsSinceEpoch);
    } else {
      typing.stopped(chat, device);
    }
    _typingChanged();
  }

  /// Tells a screen reader the others' messages that came into [group]'s room since it
  /// was last read, while its chat is on screen; none at the first reading, which is the
  /// chat as it already was.
  void _announceNewLines(String group, UiRoom? r) {
    if (r == null) return;
    final onScreen =
        nav.page == Screen.room &&
        !nav.settingsOpen &&
        windowShown.value &&
        roomChatShown &&
        roomState.roomTab == 0;
    if (!onScreen) return;
    final come = arrivals(
      roomState.memories[group],
      r,
      (line) => _nameIn(group, line.sender, nick: line.nick),
    );
    toldOf(come).forEach(announce);
  }

  String _nameIn(String group, String? device, {String? nick}) {
    for (final p in roomState.room?.participants ?? const <UiParticipant>[]) {
      if (p.person.id == device) return p.person.name;
    }
    for (final m in contacts.members[group] ?? const <UiMember>[]) {
      if (m.person.id == device) return m.person.name;
    }
    return nick ?? l10n.appSomeone;
  }

  /// Someone declined an invitation of this device: a toast, and the bell.
  void _onDeclined(UiEvent e) {
    final device = e.device;
    final id = e.group;
    if (device == null || id == null) return;
    final person =
        knownPerson(device) ??
        UiPerson(
          id: device,
          name: e.text ?? l10n.appSomeone,
          short: '',
          avatar: const UiAvatar(preset: 0),
        );
    final declined = DeclinedInvitation(
      id,
      contacts.group(id)?.name ?? e.name ?? l10n.appAGroup,
      person,
      DateTime.now().millisecondsSinceEpoch,
    );
    notices.declines.add(declined);
    say(declined.toast);
    notices.soundForNew();
  }

  void _onEvent(UiEvent e) {
    switch (e.kind) {
      // A member's or friend's new name or picture shows in the room too, and in the
      // call with them (its page, the title bar, the own card).
      case UiEventKind.groupsChanged:
        _later('groups', refreshGroups);
        _later('room', refreshRoom);
        if (callState.calls.isNotEmpty) _later('calls', refreshCalls);
      case UiEventKind.animation:
        final id = e.device;
        if (id != null) movingBook.forget(id);
      case UiEventKind.away || UiEventKind.notAway:
        contacts.away = e.kind == UiEventKind.away;
        contacts.changed();
      case UiEventKind.friendsChanged:
        _later('friends', refreshFriends);
        _later('room', refreshRoom);
        direct.friendsChanged();
        _later('direct', direct.refresh);
        if (callState.calls.isNotEmpty) _later('calls', refreshCalls);
      case UiEventKind.room:
        _onRoom(e);
      case UiEventKind.memberConnected:
        contacts.paths.putIfAbsent(e.group!, () => {})[e.device!] = e.path!;
        final carried = contacts.carriers.putIfAbsent(e.group!, () => {});
        if (e.through case final through?) {
          carried[e.device!] = through;
        } else {
          carried.remove(e.device);
        }
        contacts.changed();
      case UiEventKind.memberDisconnected:
        contacts.paths[e.group]?.remove(e.device);
        contacts.carriers[e.group]?.remove(e.device);
        if (e.device case final device?) {
          typing.gone(device);
          _typingChanged();
        }
        contacts.changed();
      case UiEventKind.friendRequest:
        say(l10n.appFriendRequest(e.name ?? l10n.appSomeone));
      case UiEventKind.friendRequestSent:
        say(l10n.appRequestDelivered(e.name ?? l10n.appSomeone));
      case UiEventKind.friendRequestFailed:
        say(explain(e.text ?? ''), error: true);
      case UiEventKind.friendAdded:
        say(l10n.appFriendsNow(e.name ?? l10n.appSomeone));
      case UiEventKind.joined:
        say(l10n.appJoinedGroup(e.name ?? ''));
      case UiEventKind.removed:
        say(l10n.appRemovedFromGroup(e.name ?? ''));
        forgetGroup(e.group!);
      case UiEventKind.groupInvited:
        say(l10n.appAddedToGroup(contacts.friendName(e.device), e.name ?? ''));
      case UiEventKind.joinAsked:
        say(
          l10n.appAsksToJoin(
            e.name ?? l10n.appSomeone,
            contacts.group(e.group)?.name ?? l10n.appAGroup,
          ),
        );
        refreshJoinAsks();
      case UiEventKind.joinAsksChanged:
        refreshJoinAsks();
      case UiEventKind.joinWaiting:
        say(l10n.appJoinWaiting(e.name ?? ''));
      case UiEventKind.invitationDeclined:
        _onDeclined(e);
      case UiEventKind.joinDeclined:
        say(l10n.appJoinDeclined(e.name ?? ''), error: true);
      case UiEventKind.download:
        if (e.group == roomState.roomGroup) _later('room', refreshRoom);
      case UiEventKind.downloadDone:
        say(l10n.appDownloadedTo(e.text ?? ''));
        if (e.group == roomState.roomGroup) _later('room', refreshRoom);
      case UiEventKind.downloadFailed:
        say(l10n.appDownloadFailed(explain(e.text ?? '')), error: true);
        if (e.group == roomState.roomGroup) _later('room', refreshRoom);
      case UiEventKind.signal:
        final call = this.call;
        if (call != null && call.group == e.group) {
          call.onSignal(e.device!, e.text ?? '');
        } else if (call == null && e.group != null && e.group == _noCallIn) {
          _tellNoCall(e.group!, e.device!);
        }
      case UiEventKind.signalsLost:
        _onSignalsLost(e);
      case UiEventKind.callIncoming || UiEventKind.callsChanged:
        refreshCalls();
      case UiEventKind.callAccepted:
        final memory = roomState.memories[e.group];
        if (memory != null) memory.othersExpected = true;
        _onCallAccepted(e.group!);
      case UiEventKind.callEnded:
        _onCallEnded(e);
      case UiEventKind.callAddEnded:
        _onAddEnded(e);
      case UiEventKind.notice:
        if (e.text case final text?) {
          say(AppState.coreNotice(text), error: true);
        }
      case UiEventKind.stopped:
        stopping.onStopped();
      case UiEventKind.otherSession:
        say(l10n.appOtherSession, error: true);
        notices.otherSessionMs = DateTime.now().millisecondsSinceEpoch;
        notices.otherSessionText = l10n.appOtherSession;
        notices.soundForNew();
        notifyListeners();
      case UiEventKind.direct ||
          UiEventKind.directMessage ||
          UiEventKind.directDownload ||
          UiEventKind.directDownloadFailed:
        if (e.kind == UiEventKind.directMessage && e.device != null) {
          typing.stopped(TypingBoard.direct(e.device!), e.device!);
          _typingChanged();
        }
        direct.onEvent(e);
      case UiEventKind.typing || UiEventKind.stoppedTyping:
        _onTyping(e);
      case UiEventKind.friendOnline || UiEventKind.friendOffline:
        if (e.kind == UiEventKind.friendOffline && e.device != null) {
          typing.gone(e.device!);
          _typingChanged();
        }
        _later('direct', direct.refresh);
      case UiEventKind.takenOut:
        _onTakenOut(e);
      case UiEventKind.updateChanged:
        updates.refresh();
      case UiEventKind.info:
        // The core's notes are for its log, but for why someone let in did not get in.
        if (e.text case final text? when notLetIn(text) != null) {
          say(notLetIn(text)!, error: true);
        }
      case UiEventKind.address || UiEventKind.hardNat:
        break;
    }
  }

  /// This device left a voice room it was in because a manager took it out or keeps it out:
  /// the core let it out of it; the window does the same and says who did it.
  Future<void> _onTakenOut(UiEvent e) async {
    final group = e.group;
    final voice = e.voice;
    if (group == null || voice == null) return;
    final name = Contacts.voiceName(contacts.voiceRoom(group, voice));
    if (roomState.roomGroup == group && roomState.roomVoice == voice) {
      if (call?.group == group) await _endCall();
      _play(UiSound.leave);
      roomState.memories.remove(group);
      roomState.outgoing.remove(group);
      roomState.dropPreviews();
      _dropVideos();
      roomState.roomGroup = null;
      roomState.room = null;
      roomState.roomVoice = AppState.firstRoom;
      // Back on the group's page: the rest of the group is as before.
      selectGroup(group);
    }
    final by = e.device;
    say(
      AppState.takenOutText(
        by == null ? null : nameOf(by, l10n.appSomeone),
        name,
        e.barrier,
      ),
    );
    await refreshGroups();
  }

  void _onRoom(UiEvent e) {
    final group = e.group!;
    final change = e.room!;
    // A message, or leaving, ends someone's typing; a room that closed has nobody typing.
    final chat = TypingBoard.room(group, e.voice ?? AppState.firstRoom);
    switch (change.kind) {
      case UiRoomChangeKind.chat ||
          UiRoomChangeKind.left ||
          UiRoomChangeKind.away:
        if (change.device case final device?) typing.stopped(chat, device);
        _typingChanged();
      case UiRoomChangeKind.closed:
        typing.clear(chat);
        _typingChanged();
      default:
        break;
    }
    // Another voice room of the group than the one shown or this device is in: its people
    // show in the lists, nothing else.
    final mine = group == roomState.roomGroup
        ? roomState.roomVoice
        : contacts.joinedVoice(group);
    if (e.voice != (mine ?? AppState.firstRoom)) {
      _later('groups', refreshGroups);
      return;
    }
    final now = DateTime.now().millisecondsSinceEpoch;
    _soundFor(group, change, now);
    // A call's room is not among the groups: the room shown says when it started.
    final startedMs =
        contacts.group(group)?.room?.startedMs ??
        (group == roomState.roomGroup ? roomState.room?.startedMs : null);
    final memory = startedMs == null
        ? null
        : roomState.memoryFor(group, startedMs);
    switch (change.kind) {
      case UiRoomChangeKind.joined:
        if (memory != null && change.device != contacts.profile?.person.id) {
          memory.others.add(change.device!);
          memory.addNote(
            RoomNote.joined(
              now,
              change.device,
              _nameIn(group, change.device, nick: change.nick),
            ),
          );
        }
      case UiRoomChangeKind.left:
        if (memory != null && change.device != contacts.profile?.person.id) {
          memory.addNote(
            RoomNote.left(now, change.device, _nameIn(group, change.device)),
          );
        }
      case UiRoomChangeKind.fileShared:
        if (memory != null && !memory.files.containsKey(change.fileId)) {
          final by = _nameIn(group, change.device);
          memory.files[change.fileId!] = SeenFile(
            change.fileId!,
            change.fileName!,
            change.fileSize,
            change.device!,
            by,
          );
          memory.addNote(RoomNote.file(now, change.fileId, change.device, by));
        }
      case UiRoomChangeKind.fileGone:
        final seen = memory?.files[change.fileId];
        if (seen != null) {
          seen.goneMs = now;
          seen.goneName = seen.byName;
        }
      case UiRoomChangeKind.closed:
        roomState.memories.remove(group);
        if (group == roomState.roomGroup) roomState.dropPreviews();
      case UiRoomChangeKind.chat:
        if (memory != null && change.chat != null) {
          memory.note(change.chat!, now);
        }
      case UiRoomChangeKind.opened ||
          UiRoomChangeKind.away ||
          UiRoomChangeKind.back:
        break;
    }
    if (group == roomState.roomGroup) _later('room', refreshRoom);
    roomState.changed();
  }

  /// Short sounds for the room this device is in: someone came in or left, someone
  /// wrote. Not for this device's own doings, and not for the room's past arriving just
  /// after entering it.
  void _soundFor(String group, UiRoomChange change, int now) {
    final me = contacts.profile?.person.id;
    final inRoom =
        (contacts.group(group)?.room?.joined ?? false) ||
        (roomState.roomGroup == group && (roomState.room?.joined ?? false));
    if (!inRoom || change.device == me || now < (_quietUntil[group] ?? 0)) {
      return;
    }
    switch (change.kind) {
      case UiRoomChangeKind.joined:
        _play(UiSound.join);
      case UiRoomChangeKind.left:
        _play(UiSound.leave);
      case UiRoomChangeKind.chat:
        if (change.chat?.mine ?? true) return;
        if (now - _lastMessageSound < 800) return;
        _lastMessageSound = now;
        _play(UiSound.message);
      default:
        break;
    }
  }
}
