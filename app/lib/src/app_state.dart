import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:path_provider/path_provider.dart';

import 'announce.dart';
import 'call.dart';
import 'direct_state.dart';
import 'format.dart';
import 'host.dart';
import 'input_seen.dart';
import 'l10n.dart';
import 'mic_test.dart' show micTest;
import 'moving.dart';
import 'outgoing_send.dart';
import 'photo.dart';
import 'private_file.dart';
import 'report.dart' show noteAppError;
import 'restart.dart';
import 'safe_image.dart';
import 'splash.dart';
import 'rust/api/account_move.dart' as moves;
import 'rust/api/app.dart' as api;
import 'rust/api/direct.dart' as dm;
import 'rust/api/types.dart';
import 'shown.dart';
import 'theme.dart';
import 'typing.dart';
import 'update.dart';
import 'window.dart' show relabelTray;
import 'state/piece.dart';
import 'state/account_move.dart';
import 'state/call_state.dart';
import 'state/contacts.dart';
import 'state/navigation.dart';
import 'state/notices.dart';
import 'state/room_state.dart';
import 'state/prefs.dart';
import 'state/toasts.dart';

export 'state/call_state.dart';
export 'state/contacts.dart';
export 'state/file_names.dart';
export 'state/navigation.dart';
export 'state/notices.dart';
export 'state/piece.dart';
export 'state/prefs.dart';
export 'state/room_state.dart';
export 'state/scope.dart';
export 'state/toasts.dart';

part 'app_state/calls.dart';
part 'app_state/events.dart';
part 'app_state/pictures.dart';

/// The downloads folder when the system does not say where it is: under the home folder,
/// which Windows names in `USERPROFILE` (it has no `HOME`), rather than one beside wherever
/// the app was started from, often its own folder, where nothing can be written.
@visibleForTesting
String downloadsFallback(Map<String, String> env, {required bool windows}) {
  final home = windows
      ? (env['USERPROFILE'] ?? env['HOME'])
      : (env['HOME'] ?? env['USERPROFILE']);
  if (home == null || home.isEmpty) return 'Downloads';
  return '$home${windows ? r'\' : '/'}Downloads';
}

class AppState extends ChangeNotifier {
  AppState(this.dataDir);

  /// Where Fogata keeps its data; empty when the system names no such folder, and then
  /// nothing starts.
  final String dataDir;

  /// What is said in passing at the bottom of the window.
  final toasts = ToastBoard();

  /// The bell, missed calls and the other notices.
  late final notices = Notices(
    contacts,
    direct,
    prefs,
    () => _play(UiSound.notice),
  );

  /// Where the window is, and what floats over it.
  final nav = Navigation();

  /// This device's profile, its groups, friends, requests and invitations.
  final contacts = Contacts();

  /// The room shown, what this window learnt of rooms, and the files waiting to go.
  final roomState = RoomState();

  /// The call of the room this device is in, and the direct calls.
  final callState = CallState();

  /// What the person chose on this computer, kept in `app.json`.
  late final prefs = Prefs(dataDir)..noticesNow = _noticeKeysNow;

  /// The small window shown while Fogata starts.
  final splash = Splash();

  /// Where updates of Fogata stand. A version that becomes ready while the main window shows is
  /// announced once, with the title bar's pill to restart into it; at start the splash
  /// installs it instead.
  late final updates = Updates(
    onArrived: (status) {
      if (splash.showing) return;
      final version = status.releaseVersion ?? '';
      say(
        status.required_
            ? l10n.updateRequiredTip
            : l10n.updateReadyTip(version),
      );
    },
  );

  bool hasProfile = false;
  bool started = false;
  String? startError;

  /// Where the data folder stands with moves of the account to another computer: moved
  /// away (or moving), or an account that came here and waits to be finished.
  moves.UiMoveState? moveState;

  /// A move under way, from here or to here.
  AccountMove? accountMove;

  /// The new computer's card shows instead of the welcome page's profile card; with
  /// [replacing], over this computer's own profile, whose core stopped.
  bool bringing = false;
  bool replacing = false;

  /// The welcome page shows the card that restores a recovery copy instead of its profile
  /// card.
  bool restoring = false;

  /// The core stopped for a move: nothing of the account changes meanwhile.
  bool offlineForMove = false;

  Future<void> loadMoveState() async {
    try {
      moveState = await moves.moveState(dataDir: dataDir);
    } catch (_) {}
  }

  /// Stops the core for a move: it lets go of the data folder, and nobody sees this
  /// computer online meanwhile.
  Future<void> stopForMove() async {
    if (!started || offlineForMove) return;
    offlineForMove = true;
    notifyListeners();
    // A room or call this device is in is left first, as when quitting; the core itself
    // stops once, below.
    final group = contacts.joinedGroup?.id;
    final direct = callState.activeCall?.groupId;
    await _endCall();
    try {
      if (direct != null) await api.endCall(groupId: direct);
      if (group != null) await api.leaveRoom(groupId: group);
    } catch (_) {}
    // The core ends the events' stream as it stops; only then can the subscription go
    // (cancelling a stream that waits for its next event waits for that event).
    final events = _events;
    _events = null;
    _firewallCheck?.cancel();
    // The calls go with the core; a ring started again after it starts anew.
    callState
      ..calls = []
      ..ring();
    // A voice message being recorded goes with the core it would have gone through.
    this.direct.voice.cancel();
    try {
      await api.stopCore(dataDir: dataDir);
    } catch (e) {
      // The data folder is still held: the move that follows cannot take it yet.
      say(explain(e), error: true);
    }
    unawaited(events?.cancel());
    // How members were reached, and who was typing, went with the core: the next one says
    // anew whom it reaches.
    contacts.paths.clear();
    contacts.carriers.clear();
    contacts.changed();
    typing.clearAll();
    _typingChanged();
  }

  /// Tells, in the bell, what was found about this computer at start: settings that could
  /// not be read ([Prefs.setAsideMs]), secrets kept unencrypted for want of a keychain or
  /// not all encrypted yet (which Settings says too). [start] calls it.
  @visibleForTesting
  void warnAtStart() {
    void warn(Warning w) {
      notices.warnings
        ..removeWhere((o) => o.key == w.key)
        ..add(w);
    }

    if (atRest == UiAtRest.noKeychain) {
      warn(
        Warning(
          'keychain',
          l10n.settingsNotEncrypted,
          l10n.settingsNotEncryptedText(onLinux ? 'linux' : 'other'),
          DateTime.now().millisecondsSinceEpoch,
        ),
      );
    } else {
      notices.warnings.removeWhere((w) => w.key == 'keychain');
    }
    // Apart from the one above: dismissing that one does not silence this one.
    if (atRest == UiAtRest.unfinished) {
      warn(
        Warning(
          'sealing',
          l10n.settingsNotAllEncrypted,
          l10n.settingsNotAllEncryptedText,
          DateTime.now().millisecondsSinceEpoch,
        ),
      );
    } else {
      notices.warnings.removeWhere((w) => w.key == 'sealing');
    }
    if (prefs.setAsideMs case final at?) {
      warn(
        Warning(
          'settings:$at',
          l10n.bellSettingsResetTitle,
          l10n.bellSettingsResetText,
          at,
        ),
      );
    }
    notices.changed();
  }

  /// Follows the core's events: [start] does, and tests give a stream of their own.
  @visibleForTesting
  set eventsSubscription(StreamSubscription<UiEvent>? events) =>
      _events = events;

  /// Follows [events], the core's. Should they end, or fail, while followed, nothing
  /// would reach the window any more: it says the network part stopped, as the core does
  /// when it stops by itself ([Stopping]). A stop for a move lets go of them first.
  @visibleForTesting
  /// Follows the core's [events]; [taken] tells the core each one was taken, so that it
  /// gives more (it gives a few hundred at most before).
  void listenTo(Stream<UiEvent> events, {void Function()? taken}) {
    late final StreamSubscription<UiEvent> followed;
    void ended() {
      if (!_disposed && identical(_events, followed)) stopping.onStopped();
    }

    followed = events.listen(
      (e) {
        try {
          _onEvent(e);
        } finally {
          taken?.call();
        }
      },
      onError: (Object e) {
        noteAppError(dataDir, 'the core\'s events failed: $e', null);
        ended();
      },
      onDone: ended,
    );
    _events = followed;
  }

  /// What stopped the core to work on the data folder with a recovery copy (making one, or
  /// deleting the account a restore replaced), until it starts it again: only that one
  /// starts it, and neither a move nor another such piece of work begins meanwhile.
  Object? stoppedBy;

  /// Stops the core for [who]'s work on the data folder; false, with nothing stopped, while
  /// another piece of work or a move holds it. [who] starts it again with
  /// [resumeAfterWork], whatever happened meanwhile.
  Future<bool> stopForWork(Object who) async {
    if (stoppedBy != null || offlineForMove) return false;
    stoppedBy = who;
    notifyListeners();
    await stopForMove();
    return true;
  }

  /// Starts the core again after [who]'s work; nothing when something else stopped it.
  Future<void> resumeAfterWork(Object who) async {
    if (!identical(stoppedBy, who)) return;
    stoppedBy = null;
    try {
      await resumeAfterMove();
    } finally {
      notifyListeners();
    }
  }

  /// Starts the core again after a move that left the account here.
  Future<void> resumeAfterMove() async {
    if (!offlineForMove) return;
    offlineForMove = false;
    await start();
  }

  void toggleBell(Rect? from) {
    notices.bellFrom = notices.bellFrom == null ? from : null;
    if (notices.bellFrom != null) {
      notices.newSince = prefs.noticesSeenMs;
      prefs.noticesSeenMs = DateTime.now().millisecondsSinceEpoch;
      // Kept for the next start: what the bell showed then is not new again.
      prefs.save();
      nav.userPanelFrom = null;
      nav.friendCard = null;
      nav.friendCardFrom = null;
    }
    notices.changed();
    nav.changed();
  }

  /// How this device's secrets are kept on this computer: unencrypted when no system
  /// keychain kept their key (the settings say so). Known once started.
  UiAtRest? atRest;

  /// The name of the secret that keeps the data from being all encrypted, damaged on the
  /// disk, when that is what keeps it ([UiAtRest.unfinished]).
  String? damagedSecret;

  /// Sets [damagedSecret] aside at the next start, which takes back its copy from before
  /// encrypting and finishes, and starts Fogata again for it.
  Future<void> setDamagedSecretAside() async {
    // Said when it cannot be noted (the file opens again, say), and Fogata goes on.
    if (await succeeds(api.setDamagedSecretAside)) await stopping.restart();
  }

  /// Direct chats with friends.
  late final direct = DirectChats(this);

  /// The parts told apart from the rest, each told only of its own changes; a change
  /// told here, to the whole state, reaches all of them.
  late final List<Piece> _pieces = [
    direct,
    prefs,
    toasts,
    notices,
    callState,
    callState.live,
    roomState,
    contacts,
    nav,
    splash,
    updates,
  ];

  /// The network part stopping by itself, and starting again.
  late final stopping = Stopping(dataDir);

  /// Looks again whether the firewall still lets nothing in.
  Timer? _firewallCheck;

  /// Asks the core what keeps others from reaching this computer, as far as its node saw
  /// (see why_unreached.dart); tests give their own.
  Future<UiReach> Function() reachability = api.reachability;

  /// The id every group's first voice room has.
  static const firstRoom = RoomState.firstRoom;

  /// Who is typing in the chats, from the signals that come; never kept.
  final typing = TypingBoard();
  Timer? _typingEnd;

  /// Ticks each time someone starts or stops showing as typing: only the lines that say so
  /// listen, rather than the whole window.
  final typingChanged = ValueNotifier<int>(0);

  /// Who is typing in the chat of the room shown, by id.
  List<String> typingInRoom() {
    final group = roomState.roomGroup;
    if (group == null) return const [];
    return typing.who(
      TypingBoard.room(group, roomState.roomVoice),
      DateTime.now().millisecondsSinceEpoch,
    );
  }

  /// Tells the others in the room's chat that this device is typing there, or stopped;
  /// nothing while it appears offline.
  void roomTyping(bool on) {
    final group = roomState.roomGroup;
    if (group == null || !started || prefs.appearOffline) return;
    api.roomTyping(groupId: group, typing: on).catchError((_) {});
  }

  /// Someone started or stopped showing as typing: the screens look again, and again when
  /// the next one's time runs out.
  void _typingChanged() {
    _typingEnd?.cancel();
    _typingEnd = null;
    final end = typing.nextEnd;
    if (end != null) {
      final wait = end - DateTime.now().millisecondsSinceEpoch;
      _typingEnd = Timer(Duration(milliseconds: wait < 0 ? 0 : wait), () {
        typing.prune(DateTime.now().millisecondsSinceEpoch);
        _typingChanged();
      });
    }
    if (!_disposed) typingChanged.value++;
  }

  /// On a small window the chat beside a room or call starts hidden. Shown by hand, it
  /// stays shown for that room's call however the window changes, until hidden by hand;
  /// the next call starts with it hidden again.
  bool get roomChatSmall =>
      roomState.chatSmallFor != null &&
      roomState.chatSmallFor == (call ?? roomState.roomGroup);
  set roomChatSmall(bool on) =>
      roomState.chatSmallFor = on ? (call ?? roomState.roomGroup) : null;

  /// Whether the chat and files are shown beside the room or call.
  bool get roomChatShown =>
      roomState.roomSmall ? roomChatSmall : prefs.roomChat;

  /// The call of the room this device is in (see [CallState.call]).
  Call? get call => callState.call;

  StreamSubscription<UiEvent>? _events;
  final Map<String, Timer> _soon = {};

  /// Until when (ms) a room's events make no sound: right after entering, what the others
  /// send is the room's past, not news.
  final Map<String, int> _quietUntil = {};
  int _lastMessageSound = 0;

  /// The last sounds played, for tests: no more than [_playedKept], since it is kept in a
  /// release build too.
  @visibleForTesting
  final List<UiSound> played = [];
  static const _playedKept = 64;

  Future<void> boot() async {
    if (!prefs.loaded) await prefs.load();
    unawaited(relabelTray());
    if (prefs.audioOutput != null) unawaited(soundsTo(prefs.audioOutput));
    // Test copies save into a folder of their own; a release build never moves it.
    if (!kReleaseMode) {
      prefs.downloadsDir = Platform.environment['FOGATA_DOWNLOADS'];
    }
    try {
      prefs.downloadsDir ??= (await getDownloadsDirectory())?.path;
    } catch (_) {}
    prefs.downloadsDir ??= downloadsFallback(
      Platform.environment,
      windows: onWindows,
    );
    await loadMoveState();
    final move = moveState?.kind ?? 'none';
    // Moved away: the app is not run here any more.
    if (move == 'moved' || move == 'moving') {
      notifyListeners();
      return;
    }
    try {
      hasProfile = await api.hasProfile(dataDir: dataDir);
    } catch (e) {
      startError = explain(e);
    }
    // An account came here and waits to be finished: its card shows, over this
    // computer's own profile if it has one, whose core starts only if it is thrown away.
    if (move == 'staged') {
      bringing = true;
      replacing = hasProfile;
      offlineForMove = hasProfile;
      notifyListeners();
      return;
    }
    if (hasProfile) await start();
    notifyListeners();
  }

  Future<void> setTheme(ThemeMode mode) => setLook(switch (mode) {
    ThemeMode.light => Look.light,
    ThemeMode.system => Look.system,
    ThemeMode.dark => Look.dark,
  });

  Future<void> setLook(Look look) async {
    prefs.look = look;
    prefs.changed();
    await prefs.save();
  }

  Future<void> setLanguage(String code) async {
    prefs.language = code;
    prefs.languageChosen = true;
    prefs.changed();
    unawaited(relabelTray());
    await prefs.save();
  }

  /// Clock times in 12 hours with AM and PM, or in 24.
  Future<void> setClock12(bool on) async {
    prefs.clock12 = on;
    prefs.changed();
    await prefs.save();
  }

  /// Notes [emoji] (its default form) as picked last.
  Future<void> useEmoji(String emoji) async {
    prefs.recentEmoji = [
      emoji,
      ...prefs.recentEmoji.where((e) => e != emoji),
    ].take(maxRecentEmoji).toList();
    await prefs.save();
  }

  Future<void> setEmojiTone(int tone) async {
    prefs.emojiTone = tone.clamp(0, 5);
    prefs.changed();
    await prefs.save();
  }

  /// Shows or hides the chat and files beside rooms and calls, by hand. Shown on a small
  /// window, they are shown on wide ones too; hidden on a wide window, on small ones too.
  /// Hidden on a small window, wide ones keep showing them.
  Future<void> setRoomChat(bool on) async {
    final remembered = prefs.roomChat;
    if (on) {
      prefs.roomChat = true;
      if (roomState.roomSmall) roomChatSmall = true;
    } else if (roomState.roomSmall) {
      roomChatSmall = false;
    } else {
      prefs.roomChat = false;
      roomChatSmall = false;
    }
    prefs.changed();
    roomState.changed();
    if (prefs.roomChat != remembered) await prefs.save();
  }

  Future<void> setSounds(bool on) async {
    prefs.sounds = on;
    prefs.changed();
    await prefs.save();
    if (on) _play(UiSound.message);
  }

  Future<void> setTray(bool on) async {
    prefs.tray = on;
    prefs.changed();
    await prefs.save();
  }

  Future<void> setBridge(bool on) async {
    prefs.bridge = on;
    prefs.changed();
    await prefs.save();
  }

  Future<void> setAppearOffline(bool offline) async {
    prefs.appearOffline = offline;
    prefs.changed();
    await prefs.save();
    if (started) await attempt(() => api.setAppearOffline(offline: offline));
  }

  Future<void> setBannerColor(Color color) async {
    prefs.bannerColor = color;
    prefs.bannerPicture = null;
    final moved = prefs.movingBanner != null;
    prefs.movingBanner = null;
    prefs.changed();
    await prefs.save();
    try {
      if (await prefs.bannerFile.exists()) await prefs.bannerFile.delete();
      if (await prefs.movingBannerFile.exists()) {
        await prefs.movingBannerFile.delete();
      }
    } catch (_) {}
    await _shareBanner();
    if (moved) await _moveBanner();
    _forgetOwnBanner();
  }

  /// A picture over the banner's colour, and the animation that moves in its place when
  /// it is one.
  Future<void> setBannerPicture(Uint8List png, {Uint8List? moving}) async {
    prefs.bannerPicture = png;
    final moved = prefs.movingBanner != null || moving != null;
    prefs.movingBanner = moving;
    prefs.changed();
    try {
      await Directory(dataDir).create(recursive: true);
      await writePrivate(prefs.bannerFile.path, png);
      if (moving != null) {
        await writePrivate(prefs.movingBannerFile.path, moving);
      } else if (await prefs.movingBannerFile.exists()) {
        await prefs.movingBannerFile.delete();
      }
    } catch (_) {}
    await _shareBanner();
    if (moved) await _moveBanner();
    _forgetOwnBanner();
  }

  /// Opens a friend's card next to `from`, the bounds of what was clicked, or closes it if
  /// it is open. With [member], the card is a group member's who is not a friend, from
  /// [group]'s list if given.
  void toggleFriendCard(
    String id,
    Rect from, {
    bool member = false,
    String? group,
  }) {
    if (nav.friendCard == id) {
      closeFriendCard();
      return;
    }
    nav.friendCard = id;
    nav.friendCardFrom = from;
    nav.memberCard = member;
    nav.memberCardGroup = group;
    nav.userPanelFrom = null;
    notices.bellFrom = null;
    nav.changed();
    notices.changed();
    if (!member) _loadFriendCard(id);
  }

  void closeFriendCard() {
    if (nav.friendCard == null) return;
    nav.friendCard = null;
    nav.friendCardFrom = null;
    nav.memberCard = false;
    nav.memberCardGroup = null;
    nav.changed();
  }

  /// Sends a friend request to a member of a group this device is in, from their card:
  /// no link or code. [name] is the name their group shows. Whether it got there comes
  /// later, as for a link.
  Future<void> askMember(String id, String name) async {
    await attempt(() => api.askMember(deviceId: id, name: name));
    await refreshFriends();
  }

  /// What a person's voice is multiplied by on this device.
  double gainOf(String id) => callState.deafened || prefs.silenced.contains(id)
      ? 0
      : prefs.volumeOf(id) / 100;

  /// Sets how loud a person's voice plays here, in percent. While a slider is dragged
  /// [save] is false, and the value is saved when it is let go.
  Future<void> setVolume(String id, int percent, {bool save = true}) async {
    final value = percent.clamp(0, 200);
    if (value == 100) {
      prefs.volumes.remove(id);
    } else {
      prefs.volumes[id] = value;
    }
    prefs.changed();
    await call?.applyVolume(id);
    if (save) await prefs.save();
  }

  /// What the sound of a person's shared screen is multiplied by on this device.
  double screenGainOf(String id) =>
      callState.deafened || prefs.screenSilenced.contains(id)
      ? 0
      : prefs.screenVolumeOf(id) / 100;

  /// Sets how loud the sound of a person's shared screen plays here, in percent.
  Future<void> setScreenVolume(
    String id,
    int percent, {
    bool save = true,
  }) async {
    final value = percent.clamp(0, 200);
    if (value == 100) {
      prefs.screenVolumes.remove(id);
    } else {
      prefs.screenVolumes[id] = value;
    }
    prefs.changed();
    await call?.applyScreenVolume(id);
    if (save) await prefs.save();
  }

  /// Stops playing the sound of a person's shared screen here, or plays it again.
  Future<void> setScreenSilenced(String id, bool on) async {
    if (on) {
      prefs.screenSilenced.add(id);
    } else {
      prefs.screenSilenced.remove(id);
    }
    prefs.changed();
    await call?.applyScreenVolume(id);
    await prefs.save();
  }

  /// Stops playing a person's voice on this device, or plays it again.
  Future<void> setSilenced(String id, bool on) async {
    if (on) {
      prefs.silenced.add(id);
    } else {
      prefs.silenced.remove(id);
    }
    prefs.changed();
    await call?.applyVolume(id);
    await prefs.save();
  }

  /// Turns the microphone on or off: in the call, and for the next one. Turning it on
  /// while deafened hears the others again too.
  Future<void> setMic(bool on) async {
    if (on != prefs.joinWithMic) _play(on ? UiSound.unmute : UiSound.mute);
    prefs.joinWithMic = on;
    if (on && callState.deafened) {
      callState.deafened = false;
      await _applyGains();
    }
    notifyListeners();
    await call?.setMic(on);
    await prefs.save();
  }

  /// The microphone as it was before deafening, to bring back on hearing again.
  bool _micBeforeDeafen = false;

  /// Not hearing anyone, or hearing again (see [CallState.deafened]).
  Future<void> setDeafened(bool on) async {
    if (on == callState.deafened) return;
    _play(on ? UiSound.mute : UiSound.unmute);
    callState.deafened = on;
    final mic = on ? false : _micBeforeDeafen;
    if (on) _micBeforeDeafen = prefs.joinWithMic;
    final micChanges = mic != prefs.joinWithMic;
    prefs.joinWithMic = mic;
    callState.changed();
    prefs.changed();
    if (micChanges) await call?.setMic(mic);
    await _applyGains();
  }

  /// Plays everyone in the call as loud as [gainOf] and [screenGainOf] now say.
  Future<void> _applyGains() async {
    final call = this.call;
    if (call == null) return;
    for (final id in call.peers.keys.toList()) {
      await call.applyVolume(id);
      await call.applyScreenVolume(id);
    }
  }

  /// Turns the camera on or off in the call, or for entering the next room.
  Future<void> setCamera(bool on) async {
    final call = this.call;
    if (call == null) {
      callState.joinWithCamera = on;
      callState.changed();
    } else {
      await call.setCamera(on);
    }
  }

  Future<void> setAudioInput(String? id) async {
    prefs.audioInput = id;
    prefs.changed();
    await call?.useInput(id);
    await prefs.save();
  }

  Future<void> setAudioOutput(String? id) async {
    prefs.audioOutput = id;
    prefs.changed();
    await soundsTo(id);
    await call?.useOutput(id);
    await prefs.save();
  }

  /// Turns noise suppression on or off, for the call now and the next ones.
  Future<void> setNoiseSuppression(bool on) async {
    prefs.noiseSuppression = on;
    prefs.changed();
    final call = this.call;
    if (call != null) {
      await call.useNoiseSuppression(on);
    } else if (micTest.on) {
      // The microphone test sounds as a call would.
      await suppressNoise(on);
    }
    await prefs.save();
  }

  Future<void> setVideoInput(String? id) async {
    prefs.videoInput = id;
    prefs.changed();
    await call?.useCamera(id);
    await prefs.save();
  }

  /// Draws this device's own camera mirrored or as it is, wherever it shows here.
  Future<void> setMirrorCamera(bool on) async {
    prefs.mirrorCamera = on;
    prefs.changed();
    await prefs.save();
  }

  /// Sets how this device's shared screen goes out; a screen shared now changes at once.
  Future<void> setScreenQuality(ScreenQuality quality) async {
    prefs.screenQuality = quality;
    prefs.changed();
    await call?.setScreenQuality(quality);
    await prefs.save();
  }

  /// The sound of someone else writing, as in a room.
  void playMessageSound() => _play(UiSound.message);

  void _play(UiSound sound) {
    if (!prefs.sounds) return;
    played.add(sound);
    if (played.length > _playedKept) played.removeAt(0);
    try {
      api.playSound(sound: sound);
    } catch (_) {}
  }

  Future<void> start() async {
    // Videos left from a run that ended without leaving its room.
    _dropVideos();
    splash.show(SplashStep.connecting);
    try {
      contacts.profile = await api.start(dataDir: dataDir);
      started = true;
      // A core started again says away anew.
      contacts.away = false;
      watchInput();
      hasProfile = true;
      try {
        contacts.bio = await api.myBio();
      } catch (_) {}
      try {
        atRest = await api.atRest();
        if (atRest == UiAtRest.unfinished) {
          damagedSecret = await api.damagedSecret();
        }
      } catch (_) {}
      warnAtStart();
      movingBook
        ..clear()
        ..ask = _askMoving;
      listenTo(api.events(), taken: () => api.eventsTaken(count: 1));
      unawaited(updates.refresh());
      if (startedByItself) say(l10n.appRestarted);
      await Future.wait([
        refreshGroups(),
        refreshFriends(),
        refreshJoinAsks(),
        direct.refresh(),
      ]);
      notices.listen();
      await _shareBanner();
      await _shareMoving();
      try {
        notices.firewallCommand = await api.firewallCommand();
        notices.firewallSinceMs = DateTime.now().millisecondsSinceEpoch;
        notices.known = true;
      } catch (_) {}
      // The notice goes once the person has run the command (the core then tries friends
      // and members again by itself).
      if (notices.firewallCommand != null) {
        _firewallCheck = Timer.periodic(const Duration(seconds: 30), (_) async {
          try {
            if (await api.firewallCommand() == null) {
              notices.firewallCommand = null;
              _firewallCheck?.cancel();
              notices.changed();
            }
          } catch (_) {}
        });
      }
    } catch (e) {
      startError = explain(e);
    }
    notifyListeners();
  }

  /// Tries again after Fogata could not start.
  Future<void> retryStart() async {
    if (started || dataDir.isEmpty) return;
    startError = null;
    notifyListeners();
    try {
      hasProfile = await api.hasProfile(dataDir: dataDir);
    } catch (e) {
      startError = explain(e);
      notifyListeners();
      return;
    }
    if (hasProfile) {
      await start();
    } else {
      notifyListeners();
    }
  }

  @override
  void dispose() {
    _disposed = true;
    _events?.cancel();
    _firewallCheck?.cancel();
    _typingEnd?.cancel();
    _timeOuts?.cancel();
    for (final t in _soon.values) {
      t.cancel();
    }
    for (final piece in _pieces) {
      piece.dispose();
    }
    super.dispose();
  }

  bool _disposed = false;

  /// Work that was waiting on the core may end after the window let this go: it tells
  /// nobody then. Told here, every part hears it too; a change of one part only tells
  /// that part (see [Piece.changed]).
  @override
  void notifyListeners() {
    if (_disposed) return;
    for (final piece in _pieces) {
      piece.changed();
    }
    super.notifyListeners();
  }

  // ----- Toasts -----

  /// Says [text] in a toast; one with [path] can show that file or copy its path.
  void say(String text, {bool error = false, String? path}) =>
      toasts.say(text, error: error, path: path);

  /// Runs [work]; whether it worked. Tells the person what went wrong if it failed.
  Future<bool> succeeds(Future<void> Function() work) async {
    try {
      await work();
      return true;
    } catch (e) {
      say(explain(e), error: true);
      return false;
    }
  }

  /// Runs [work], telling the person what went wrong if it fails.
  Future<T?> attempt<T>(Future<T> Function() work) async {
    try {
      return await work();
    } catch (e) {
      say(explain(e), error: true);
      return null;
    }
  }

  // ----- Navigation -----

  void go(Screen to) {
    nav.friendCard = null;
    nav.friendCardFrom = null;
    nav.page = to;
    if (to == Screen.friends) {
      notices.missedUnseen = false;
      notices.missedNote = null;
    }
    nav.changed();
    notices.changed();
  }

  /// Closes the settings, after asking when they hold changes not saved; whether they
  /// closed.
  Future<bool> closeSettings() async {
    if (nav.askingToClose) return false;
    final ask = nav.settingsMayClose;
    if (ask != null) {
      nav.askingToClose = true;
      try {
        if (!await ask()) return false;
      } finally {
        nav.askingToClose = false;
      }
    }
    if (!nav.settingsOpen) return true;
    nav.settingsOpen = false;
    nav.settingsMayClose = null;
    nav.changed();
    return true;
  }

  /// Opens the settings over the page, at [section]; without one, at the part a chat's
  /// storage or the room's network notice asked for, or where they open by themselves.
  void openSettings([SettingsSection? section]) {
    nav.friendCard = null;
    nav.friendCardFrom = null;
    nav.settingsOpen = true;
    nav.settingsSection =
        section ??
        (direct.storageOnOpen
            ? SettingsSection.storage
            : nav.networkCheckOnOpen
            ? SettingsSection.network
            : null);
    nav.changed();
  }

  /// Shows a part of the settings, or the list of parts (`null`) where it is shown alone.
  void showSettingsSection(SettingsSection? section) {
    nav.settingsSection = section;
    nav.changed();
  }

  /// Opens the person's panel next to `from`, the bounds of what was clicked: above or
  /// below their own card, or [beside] their name or picture elsewhere.
  void openUserPanel(Rect from, {bool beside = false}) {
    nav.userPanelFrom = from;
    nav.userPanelBeside = beside;
    nav.friendCard = null;
    nav.friendCardFrom = null;
    nav.changed();
  }

  void closeUserPanel() {
    if (nav.userPanelFrom == null) return;
    nav.userPanelFrom = null;
    nav.changed();
  }

  /// Makes a new friend link or code (an older one stops working) and keeps it for the
  /// panel and Friends.
  Future<UiInvitation?> makeInvitation({required bool code}) async {
    final made = await attempt(
      () => code ? api.friendCode() : api.friendLink(),
    );
    if (made != null) {
      if (code) {
        contacts.friendCode = made;
      } else {
        contacts.friendLink = made;
      }
      contacts.changed();
    }
    return made;
  }

  /// Copies the friend link shown in the panel and Friends while it still has a minute
  /// left, or else a new one (which makes the older one stop working).
  Future<bool> copyFriendLink() async {
    final shown = contacts.friendLink;
    final now = DateTime.now().millisecondsSinceEpoch;
    final link = shown != null && shown.expiresMs - now > 60000
        ? shown
        : await makeInvitation(code: false);
    if (link == null) return false;
    await Clipboard.setData(ClipboardData(text: link.text));
    return true;
  }

  /// Shows a group's members.
  void showMembers(String group) {
    nav.membersGroup = group;
    nav.selectedGroup = group;
    nav.page = Screen.members;
    nav.changed();
  }

  /// After leaving a group or being removed from it: what the window held of it goes,
  /// and a page that showed it gives way to home.
  void forgetGroup(String group) {
    if (call?.group == group) _endCall();
    final shown =
        nav.selectedGroup == group ||
        (nav.page == Screen.room && roomState.roomGroup == group) ||
        (nav.page == Screen.members && nav.membersGroup == group);
    if (roomState.roomGroup == group) {
      roomState.roomGroup = null;
      roomState.room = null;
      nav.before = null;
      roomState.dropPreviews();
      _dropVideos();
    }
    roomState.memories.remove(group);
    roomState.outgoing.remove(group);
    contacts.paths.remove(group);
    contacts.carriers.remove(group);
    if (nav.groupSettings == group) nav.groupSettings = null;
    if (nav.membersGroup == group) nav.membersGroup = null;
    if (shown) {
      goHome();
    } else {
      notifyListeners();
    }
  }

  /// Home in the rail: the overview, friends and settings.
  void goHome() {
    nav.selectedGroup = null;
    nav.page = Screen.home;
    nav.changed();
  }

  /// A group in the rail: its room if this device is in it, else its page.
  void selectGroup(String group) {
    if (roomState.roomGroup == group &&
        (contacts.group(group)?.room?.joined ?? false)) {
      _noteWayBack();
      nav.selectedGroup = group;
      nav.page = Screen.room;
    } else {
      nav.selectedGroup = group;
      nav.membersGroup = group;
      nav.page = Screen.members;
    }
    nav.changed();
  }

  /// Shows the room's chat (0) or files (1), entering the room if needed, and brings
  /// them back if they were hidden.
  Future<void> showRoomTab(String group, int tab) async {
    roomState.roomTab = tab;
    // Asked for, so shown at any size: in this room's call, or in the one it opens.
    roomState.chatSmallFor = call?.group == group ? call : group;
    if (!prefs.roomChat) unawaited(setRoomChat(true));
    if (roomState.roomGroup == group && nav.page == Screen.room) {
      roomState.changed();
      return;
    }
    await openRoom(group);
  }

  /// The room shown is a direct call's, going on or just ended.
  bool get inCallPage =>
      nav.page == Screen.room &&
      roomState.roomGroup != null &&
      (callState.callIn(roomState.roomGroup) != null ||
          callState.callEnding?.group == roomState.roomGroup);

  /// Each refresh of the calls, the groups and the room is numbered as it is asked for: an
  /// answer that comes after a later one's is dropped, or it would show what was before.
  int _callsAsked = 0, _callsShown = 0;
  int _groupsAsked = 0, _groupsShown = 0;
  int _roomAsked = 0, _roomShown = 0;

  Future<void> refreshCalls() async {
    final asked = ++_callsAsked;
    try {
      final calls = await api.calls();
      final returns = (await api.callReturns()).toSet();
      if (asked < _callsShown) return;
      _callsShown = asked;
      callState.calls = calls;
      callState.callReturns = returns;
    } catch (_) {}
    callState.ring();
    callState.changed();
  }

  /// The voice room this device and `friend` are both in, by its name; null when they
  /// are not together. A friend in the same room is not rung: you can already talk.
  String? sharedRoom(String friend) {
    final g = contacts.joinedGroup;
    if (g == null) return null;
    final voice = contacts.joinedVoice(g.id);
    final v = voice == null ? null : contacts.voiceRoom(g.id, voice);
    final people = v?.room?.participants ?? const <UiPerson>[];
    if (!people.any((p) => p.id == friend)) return null;
    return g.voiceRooms.length > 1
        ? l10n.appVoiceRoomInGroup(Contacts.voiceName(v), g.name)
        : g.name;
  }

  /// Rings a friend, leaving the room this device is in. The room's call goes on until
  /// the new one starts, so a call that cannot be made leaves it as it was.
  Future<void> callFriend(String friend) async {
    if (sharedRoom(friend) case final room?) {
      say(l10n.appBothIn(room));
      return;
    }
    callState.callEnding = null;
    final group = await attempt(() => api.startCall(deviceId: friend));
    await refreshCalls();
    // Gone already when the friend's own call crossed it and was answered instead. The
    // friend's call that rang here is answered instead of ringing back.
    final call = callState.callIn(group);
    if (group == null || call == null) return;
    final answered = call.phase == UiCallPhase.connected;
    if (answered) _play(UiSound.join);
    await _enterCall(group, answered: answered);
  }

  /// Answers the call ringing here, leaving the room this device is in; `micOff`, when
  /// given, enters with the microphone off or on, and otherwise as it was left.
  Future<void> acceptCall(UiCall c, {bool? micOff}) async {
    if (micOff != null) prefs.joinWithMic = !micOff;
    callState.callEnding = null;
    callState.hush(c.groupId);
    final ok = await succeeds(() => api.acceptCall(groupId: c.groupId));
    if (!ok) callState.unhush(c.groupId);
    await refreshCalls();
    if (!ok) return;
    _play(UiSound.join);
    await _enterCall(c.groupId, answered: true);
  }

  /// Moves the call ringing in the middle of the window to its corner; it rings on.
  void putRingAside(UiCall c) {
    callState.ringAside = c.groupId;
    callState.changed();
  }

  Future<void> declineCall(UiCall c) async {
    callState.hush(c.groupId);
    if (!await succeeds(() => api.declineCall(groupId: c.groupId))) {
      callState.unhush(c.groupId);
    }
    await refreshCalls();
  }

  /// Stops ringing, or hangs up, and goes home.
  Future<void> hangUp() async {
    final group = roomState.roomGroup;
    if (group == null) return;
    callState.hush(group);
    await _endCall();
    if (callState.callIn(group) != null) {
      _play(UiSound.callEnded);
      await attempt(() => api.endCall(groupId: group));
    }
    _leaveCallPage();
  }

  /// Rings a friend into the call this device is in.
  Future<void> addToCall(String friend) async {
    final group = callState.activeCall?.groupId;
    if (group == null) return;
    await attempt(() => api.addToCall(groupId: group, deviceId: friend));
    await refreshCalls();
  }

  /// Stops ringing a friend into the call.
  Future<void> cancelAdd(String friend) async {
    final group = callState.activeCall?.groupId;
    if (group == null) return;
    await attempt(() => api.cancelAdd(groupId: group, deviceId: friend));
    await refreshCalls();
  }

  /// Who else is in a call, by name: those in its room while it is shown, or else the
  /// friend it rang or that rang.
  String callNames(UiCall c) {
    final room = roomState.roomGroup == c.groupId ? roomState.room : null;
    final inRoom = [
      for (final p in room?.participants ?? const <UiParticipant>[])
        if (!p.me) p.person.name,
    ];
    return inRoom.isEmpty ? c.with_.name : inRoom.join(', ');
  }

  /// After a call that did not go through: rings the same friend again.
  Future<void> callAgain() async {
    final ending = callState.callEnding;
    if (ending == null) return;
    await callFriend(ending.person.id);
  }

  /// Leaves the page of a call that did not go through.
  void closeCallPage() => _leaveCallPage();

  /// Shows the call this device is in, from elsewhere in the window.
  void showCall() {
    final call = callState.activeCall;
    if (call == null) return;
    _noteWayBack();
    roomState.roomGroup = call.groupId;
    roomState.roomVoice = firstRoom;
    nav.selectedGroup = null;
    nav.page = Screen.room;
    nav.changed();
    roomState.changed();
  }

  /// Notes the page shown before the window turns to a room or a call. Going from one
  /// room or call to another keeps the first page.
  void _noteWayBack() {
    if (nav.page == Screen.room) return;
    nav.before = (
      page: nav.page,
      selected: nav.selectedGroup,
      members: nav.membersGroup,
    );
  }

  /// After leaving a room or a call: back to the page shown before it if the room or call
  /// was shown, or home if that page's group is gone.
  void _goBack() {
    final back = nav.before;
    nav.before = null;
    if (nav.page != Screen.room) return;
    final gone =
        back?.selected != null && contacts.group(back!.selected) == null;
    if (back == null || gone) {
      nav.selectedGroup = null;
      nav.page = Screen.home;
      return;
    }
    nav.page = back.page;
    nav.selectedGroup = back.selected;
    nav.membersGroup = back.members;
  }

  // ----- Refreshing -----

  void _later(String key, Future<void> Function() work) {
    _soon[key]?.cancel();
    _soon[key] = Timer(const Duration(milliseconds: 120), () => work());
  }

  Future<void> refreshGroups() async {
    final asked = ++_groupsAsked;
    try {
      // The voice room shown, to tell if a manager deletes it while this device is in it.
      final shown = callState.callIn(roomState.roomGroup) == null
          ? contacts.voiceRoom(roomState.roomGroup, roomState.roomVoice)
          : null;
      final groups = await api.groups();
      final invitations = await api.groupInvitations();
      // Asked for together, not one group after the other. One that cannot be read (the
      // group went between the two questions) keeps the members seen, and the rest is
      // taken all the same.
      final rosters = await Future.wait([
        for (final g in groups)
          api
              .members(groupId: g.id)
              .then<List<UiMember>?>((roster) => roster)
              .onError((_, _) => null),
      ]);
      if (asked < _groupsShown) return;
      _groupsShown = asked;
      contacts.groups = groups;
      contacts.invitations = invitations;
      final now = DateTime.now().millisecondsSinceEpoch;
      for (final i in contacts.invitations) {
        notices.invitationSeen.putIfAbsent(i.id, () => now);
      }
      for (final (i, g) in contacts.groups.indexed) {
        final roster = rosters[i];
        if (roster != null) contacts.members[g.id] = roster;
      }
      // A group this device left keeps no members here: their names there are old ones.
      contacts.members.removeWhere((id, _) => contacts.group(id) == null);
      contacts.groupBanners.removeWhere((id, _) => contacts.group(id) == null);
      contacts.forgetGonePeople();
      roomState.rememberRooms(contacts.groups);
      if (shown != null) await leaveIfDeleted(shown);
      _followCall();
      _timeOutsTick();
    } catch (_) {}
    notices.soundForNew();
    notifyListeners();
  }

  Timer? _timeOuts;

  /// Whether the time outs are being watched, as [timeOutsTick] arms it.
  @visibleForTesting
  bool get watchingTimeOuts => _timeOuts?.isActive ?? false;

  @visibleForTesting
  void timeOutsTick() => _timeOutsTick();

  /// While a time out is on (this device's, or one a manager sees in the settings), looks
  /// again each minute, and when the nearest runs out: the time left goes down, and a
  /// room opens again by itself.
  void _timeOutsTick() {
    _timeOuts?.cancel();
    final now = DateTime.now().millisecondsSinceEpoch;
    final ends = [
      for (final g in contacts.groups) ...[
        for (final v in g.voiceRooms) ?v.barrier?.untilMs,
        for (final k in g.keptOut ?? const <UiKeptOut>[]) ?k.untilMs,
      ],
    ].where((until) => until > now);
    if (ends.isEmpty) return;
    final next = ends.reduce((a, b) => a < b ? a : b) - now;
    _timeOuts = Timer(
      Duration(milliseconds: next.clamp(1000, 60 * 1000) + 50),
      refreshGroups,
    );
  }

  /// The voice room shown is no longer in its group, deleted by a manager with this device
  /// in it: the core let it out of it; the window does the same.
  @visibleForTesting
  Future<void> leaveIfDeleted(UiVoiceRoom shown) async {
    final group = roomState.roomGroup;
    if (group == null ||
        contacts.group(group) == null ||
        roomState.roomVoice != shown.id ||
        contacts.voiceRoom(group, shown.id) != null) {
      return;
    }
    if (call?.group == group) await _endCall();
    roomState.memories.remove(group);
    roomState.outgoing.remove(group);
    roomState.dropPreviews();
    _dropVideos();
    roomState.roomGroup = null;
    roomState.room = null;
    roomState.roomVoice = firstRoom;
    _goBack();
    say(l10n.appVoiceRoomDeleted(Contacts.voiceName(shown)));
  }

  /// The call in which each input (by id, '' for the system's default) was last said to
  /// read nothing, this session.
  final Map<String, Call> _micUnusableIn = {};

  /// The microphone of [call] reads nothing ([Call.onMicUnusable]): it goes off in the call
  /// only, and the preference to enter with it on stays, as the input may work again (a
  /// switch turned back on, a device plugged in). Said once a session for each input, and
  /// again in the same call when the person turns it back on and it still reads nothing;
  /// a later call with the same input turns it off without saying it again.
  @visibleForTesting
  Future<void> micUnusable(Call call, String text) async {
    final input = prefs.audioInput ?? '';
    final last = _micUnusableIn[input];
    _micUnusableIn[input] = call;
    if (last == null || last == call) say(text, error: true);
    await call.setMic(false);
    notifyListeners();
  }

  /// The room this device is in without a call (no sound server): whoever calls it there
  /// is told so ([Call.noVoice]) instead of being left trying.
  String? _noCallIn;

  /// When each one was last told, so someone trying again and again is told at most every
  /// 10 s.
  final Map<String, DateTime> _noCallTold = {};

  /// Sends a call signal through the core; tests see it instead.
  @visibleForTesting
  void Function(String group, String device, String payload) sendSignal =
      (group, device, payload) => api
          .sendSignal(groupId: group, deviceId: device, payload: payload)
          .catchError((_) {});

  /// Asks whether a call can start (see [Call.soundServer]); tests give their own answer.
  @visibleForTesting
  Future<bool> Function() soundServer = Call.soundServer;

  /// The room this device is in without a call, for tests.
  @visibleForTesting
  String? get noCallIn => _noCallIn;

  /// Ends the call as leaving the room does, for tests.
  @visibleForTesting
  Future<void> endCallForTests() => _endCall();

  /// Starts the call of [group] as entering its room does; for tests.
  @visibleForTesting
  Future<void> startCallForTests(String group) => _startCall(group);

  Future<void> refreshFriends() async {
    try {
      contacts.friends = await api.friends();
      contacts.requests = await api.requests();
      final card = nav.friendCard;
      if (card != null) {
        if (contacts.friends.any((f) => f.person.id == card)) {
          contacts.friendBanners[card] = await api.friendBanner(deviceId: card);
        } else if (!nav.memberCard) {
          // No longer a friend: removed here or by them. A member's card stays: it says
          // where a request between the two stands.
          nav.friendCard = null;
          nav.friendCardFrom = null;
        }
      }
      contacts.friendBanners.removeWhere(
        (id, _) => !contacts.friends.any((f) => f.person.id == id),
      );
      contacts.forgetGonePeople();
    } catch (_) {}
    notices.soundForNew();
    notifyListeners();
  }

  Future<void> refreshRoom() async {
    final id = roomState.roomGroup;
    if (id == null) return;
    final asked = ++_roomAsked;
    try {
      final voice = roomState.roomVoice;
      final r = await api.room(groupId: id, voice: voice);
      if (roomState.roomGroup != id || roomState.roomVoice != voice) return;
      if (asked < _roomShown) return;
      _roomShown = asked;
      roomState.room = r;
      _announceNewLines(id, r);
      roomState.remember(id, r);
      roomState.fetchPreviews(id, r);
      if (callState.callIn(id) != null) _followCall();
    } catch (_) {}
    roomState.changed();
  }

  /// Someone as known here now: this device, someone in the room shown, a member of one of
  /// the groups or a friend. A name or picture changed since shows wherever this is used.
  UiPerson? knownPerson(String? device) {
    if (device == null) return null;
    final me = contacts.profile?.person;
    if (me != null && me.id == device) return me;
    for (final p in roomState.room?.participants ?? const <UiParticipant>[]) {
      if (p.person.id == device) return p.person;
    }
    // The groups this device is in now: one it left keeps the names it knew then.
    for (final id in [
      roomState.roomGroup,
      ...contacts.groups.map((g) => g.id),
    ]) {
      for (final m in contacts.members[id] ?? const <UiMember>[]) {
        if (m.person.id == device) return m.person;
      }
    }
    for (final f in contacts.friends) {
      if (f.person.id == device) return f.person;
    }
    return null;
  }

  /// The name `device` has now, or `then`, the one it had when it was noted.
  String nameOf(String? device, String then) =>
      knownPerson(device)?.name ?? then;

  /// Takes an event of the core as if it had come from it.
  @visibleForTesting
  void onEvent(UiEvent e) => _onEvent(e);

  /// A notice of the core in words for people: the core says its known ones in English,
  /// found here by their words; others are shown as they come.
  static String coreNotice(String text) {
    if (text.startsWith('Your list of friends was damaged')) {
      return l10n.appFriendsRestored;
    }
    final ranOut = RegExp(r'^The invitation to “(.*)” ran out before')
        .firstMatch(text);
    if (ranOut != null) return l10n.appInvitationRanOut(ranOut.group(1)!);
    return text;
  }

  /// What a person taken out of voice room [room] by [by] is told.
  static String takenOutText(String? by, String room, UiBarrier? barrier) {
    final who = by ?? l10n.appSomeone;
    final since = barrier?.sinceMs;
    final until = barrier?.untilMs;
    return switch (barrier?.kind) {
      null || UiBarrierKind.removed => l10n.appTakenOut(who, room),
      UiBarrierKind.notChosen => l10n.appRoomForSome(room),
      UiBarrierKind.untilLetBack => l10n.appTakenOutUntilLetBack(who, room),
      UiBarrierKind.timedOut =>
        since != null && until != null
            ? l10n.appTakenOutFor(who, room, spanWords(until - since))
            : l10n.appTakenOutWhile(who, room),
    };
  }

  /// Why this device may not enter voice room [v] now, in words, if it may not.
  static String? barrierText(UiVoiceRoom v, {int? nowMs}) {
    final barrier = v.barrier;
    if (barrier == null) return null;
    final name = Contacts.voiceName(v);
    final until = barrier.untilMs;
    if (barrier.kind == UiBarrierKind.timedOut && until != null) {
      final now = nowMs ?? DateTime.now().millisecondsSinceEpoch;
      return l10n.appComeBackIn(name, timeLeft(until - now));
    }
    return l10n.appRoomNotForYou(name);
  }

  /// Takes [person] out of voice room [voice] of [group] now; they may come back at once.
  Future<void> removeFromRoom(
    String group,
    String voice,
    UiPerson person,
  ) async {
    await attempt(
      () =>
          api.removeFromRoom(groupId: group, voice: voice, memberId: person.id),
    );
  }

  /// Keeps [person] out of voice room [voice] of [group] for [minutes], or until a manager
  /// lets them back.
  Future<void> keepOutOfRoom(
    String group,
    String voice,
    UiPerson person,
    int? minutes,
  ) async {
    await attempt(
      () => api.keepOutOfRoom(
        groupId: group,
        voice: voice,
        memberId: person.id,
        minutes: minutes,
      ),
    );
    await refreshGroups();
  }

  /// Lets [member] back into voice room [voice] of [group].
  Future<void> letBackIntoRoom(
    String group,
    String voice,
    String member,
  ) async {
    await attempt(
      () => api.letBackIntoRoom(groupId: group, voice: voice, memberId: member),
    );
    await refreshGroups();
  }

  /// Who may enter voice room [voice] of [group]: everyone, or only [members], this device
  /// and those above it.
  Future<void> setRoomAccess(
    String group,
    String voice,
    List<String>? members,
  ) async {
    await attempt(
      () => api.setRoomAccess(groupId: group, voice: voice, members: members),
    );
    await refreshGroups();
  }

  // ----- Rooms -----

  /// Shows a group's room, entering it.
  Future<void> openRoom(String group, {String? voice}) async {
    // A direct call hangs up: this device is in one room at a time.
    final direct = callState.activeCall;
    if (direct != null) {
      await _endCall();
      await attempt(() => api.endCall(groupId: direct.groupId));
      callState.callEnding = null;
      await refreshCalls();
    }
    final target = voice ?? contacts.voiceToOpen(group);
    final before = contacts.voiceRoom(group, target)?.room;
    final alreadyIn = contacts.joinedVoice(group) == target;
    // Where the window was, to go back to if the new room does not take this device.
    final fromGroup = roomState.roomGroup;
    final fromVoice = roomState.roomVoice;
    final fromPage = nav.page;
    final fromSelected = nav.selectedGroup;
    // Another voice room of the same group: what was seen in the last one goes, once in.
    final otherVoice =
        roomState.roomGroup == group && roomState.roomVoice != target;
    if (roomState.roomGroup != group || roomState.roomVoice != target) {
      roomState.dropPreviews();
    }
    if (otherVoice && alreadyIn) roomState.memories.remove(group);
    _noteWayBack();
    roomState.roomGroup = group;
    roomState.roomVoice = target;
    nav.selectedGroup = group;
    roomState.room = null;
    nav.page = Screen.room;
    notifyListeners();
    if (!alreadyIn) {
      _quietUntil[group] = DateTime.now().millisecondsSinceEpoch + 3000;
      final camera = call?.camera ?? false;
      await _endCall();
      if (!await succeeds(() => api.joinRoom(groupId: group, voice: target))) {
        // Full, for one: the core said no before leaving the room this device was in,
        // which goes on as it was, with its call.
        await refreshGroups();
        final back =
            fromGroup != null && contacts.joinedVoice(fromGroup) == fromVoice
            ? fromGroup
            : null;
        roomState.room = null;
        roomState.roomGroup = back;
        roomState.roomVoice = back == null ? firstRoom : fromVoice;
        if (back != null && fromPage == Screen.room) {
          nav.selectedGroup = fromSelected;
        } else {
          _goBack();
        }
        notifyListeners();
        if (back != null) {
          callState.joinWithCamera = camera;
          await refreshRoom();
          await _startCall(back);
        }
        return;
      }
      if (otherVoice) roomState.memories.remove(group);
      _play(UiSound.join);
      await refreshGroups();
      final summary = contacts.voiceRoom(group, target)?.room;
      if (summary != null) {
        final memory = roomState.memoryFor(group, summary.startedMs);
        memory.joinedMs = DateTime.now().millisecondsSinceEpoch;
        memory.othersExpected = before?.participants.isNotEmpty ?? false;
        memory.othersMaybe =
            !memory.othersExpected && (contacts.group(group)?.online ?? 0) > 0;
      }
    }
    await refreshRoom();
    await _startCall(group);
  }

  /// Leaves the room shown and goes home.
  /// Before the application quits: ends the call and leaves the room, gives the message a
  /// moment to go out, and closes every connection, so the others see this device go at
  /// once.
  Future<void> leaveBeforeQuit() async {
    final group = contacts.joinedGroup?.id;
    final direct = callState.activeCall?.groupId;
    await _endCall();
    if (!started) return;
    try {
      // A direct call hangs up, which also leaves its room.
      if (direct != null) await api.endCall(groupId: direct);
      if (group != null) await api.leaveRoom(groupId: group);
      if (direct != null || group != null) {
        await Future<void>.delayed(const Duration(milliseconds: 400));
      }
      await api.shutdown();
    } catch (_) {}
  }

  Future<void> leaveRoom() async {
    final group = roomState.roomGroup;
    if (group == null) return;
    await _endCall();
    if (await succeeds(() => api.leaveRoom(groupId: group))) {
      _play(UiSound.leave);
    }
    roomState.memories.remove(group);
    roomState.outgoing.remove(group);
    roomState.dropPreviews();
    _dropVideos();
    roomState.roomGroup = null;
    roomState.room = null;
    _goBack();
    await refreshGroups();
  }

  /// Writes [text] in the chat of the room shown; whether it went. When it did not, it is
  /// kept as the room's draft unless something else was written there since.
  Future<bool> sendChat(String text) async {
    final group = roomState.roomGroup;
    final draft = roomState.draftHere;
    if (group == null || draft == null || text.trim().isEmpty) return false;
    final sent = await succeeds(
      () => api.say(groupId: group, text: text.trim()),
    );
    if (!sent && (roomState.drafts[draft] ?? '').trim().isEmpty) {
      roomState.drafts[draft] = text;
    }
    await refreshRoom();
    return sent;
  }

  // ----- Files waiting to be shared -----

  /// Adds files to send in the room, in the chat. Folders cannot be shared and are left
  /// out, and said so; a file already waiting, or being sent, is not added twice.
  Future<void> addOutgoing(Iterable<String> paths) async {
    final group = roomState.roomGroup;
    if (group == null) return;
    final (files, folders) = await readOutgoing(paths);
    final list = roomState.outgoing.putIfAbsent(group, () => []);
    final going = _sending[group] ?? const <String>[];
    for (final f in files) {
      if (!list.any((w) => w.path == f.path) && !going.contains(f.path)) {
        list.add(f);
      }
    }
    if (list.isEmpty) roomState.outgoing.remove(group);
    if (folders > 0) {
      say(l10n.appFoldersNotShared(folders));
    }
    roomState.roomTab = 0;
    roomState.changed();
  }

  void removeOutgoing(OutgoingFile file) {
    final list = roomState.outgoing[roomState.roomGroup];
    list?.remove(file);
    if (list != null && list.isEmpty) {
      roomState.outgoing.remove(roomState.roomGroup);
    }
    roomState.changed();
  }

  void clearOutgoing() {
    roomState.outgoing.remove(roomState.roomGroup);
    roomState.changed();
  }

  /// The paths of the files [sendOutgoing] is sending, by room: they are in no list
  /// meanwhile, and picked again would be shared twice.
  final _sending = <String, List<String>>{};

  /// Shares a file in the room; the tests put their own in its place.
  @visibleForTesting
  Future<String> Function({
    required String groupId,
    required String path,
    UiVideo? video,
  })
  shareFile = api.shareFile;

  /// Whether this state has been let go of.
  bool get isDisposed => _disposed;

  /// Shares the waiting files with [text], if any: the message first, so the files show
  /// under it. A file that does not go (it failed, or the room took no more files before
  /// its turn) waits again, as the direct chats' files do; one that went does not.
  /// Whether the message went (none to send counts as gone); one that did not is kept as
  /// the room's draft, as [sendChat] keeps it.
  Future<bool> sendOutgoing(String text) async {
    final group = roomState.roomGroup;
    final draft = roomState.draftHere;
    if (group == null || draft == null) return false;
    var said = true;
    final batch = roomState.outgoing.remove(group) ?? const <OutgoingFile>[];
    final paths = [for (final f in batch) f.path];
    (_sending[group] ??= []).addAll(paths);
    for (final f in batch) {
      f.state = OutgoingState.sending;
    }
    roomState.changed();
    final shared = <String>[];
    // The visit to the room the files are for: left, or left and entered again, while a
    // file was on its way, the rest are not shared there.
    final voice = roomState.roomVoice;
    final visit = roomState.memories[group];
    bool sameRoom() =>
        !_disposed &&
        roomState.roomGroup == group &&
        roomState.roomVoice == voice &&
        (visit == null || identical(roomState.memories[group], visit));
    try {
      if (text.trim().isNotEmpty) {
        said = await succeeds(() => api.say(groupId: group, text: text.trim()));
        if (!said && (roomState.drafts[draft] ?? '').trim().isEmpty) {
          roomState.drafts[draft] = text;
        }
      }
      var full = false;
      for (final f in batch) {
        // The room was left: its files go no further (a send says one error per file).
        if (!sameRoom()) {
          f.state = OutgoingState.pending;
          continue;
        }
        // The room takes no more of this person's files: the rest would only say so again.
        if (full) {
          f.state = OutgoingState.pending;
          continue;
        }
        try {
          // A video goes with its first picture and length, as this computer reads them,
          // which takes a while: the room may have been left meanwhile.
          final video = await videoFor(f);
          if (!sameRoom()) {
            f.state = OutgoingState.pending;
            continue;
          }
          await shareFile(groupId: group, path: f.path, video: video);
          f.state = OutgoingState.shared;
          shared.add(f.name);
        } catch (e) {
          f.state = OutgoingState.failed;
          final why = explain(e);
          say(why, error: true);
          if (why == explain('files in this room already')) full = true;
        }
      }
      final back = [
        for (final f in batch)
          if (f.state != OutgoingState.shared) f,
      ];
      // Still in the room they were meant for: leaving it let them go.
      if (back.isNotEmpty && roomState.roomGroup == group && !_disposed) {
        final added = roomState.outgoing[group] ?? const <OutgoingFile>[];
        roomState.outgoing[group] = [
          ...back,
          ...added.where((a) => !back.any((f) => f.path == a.path)),
        ];
        roomState.changed();
      }
    } finally {
      final going = _sending[group];
      if (going != null) {
        for (final path in paths) {
          going.remove(path);
        }
        if (going.isEmpty) _sending.remove(group);
      }
    }
    if (shared.length == 1) {
      say(l10n.appSharingOne(shared.single));
    } else if (shared.length > 1) {
      say(l10n.appSharingMany(shared.length));
    }
    await refreshRoom();
    return said;
  }

  Future<void> stopSharing(String file) async {
    final group = roomState.roomGroup;
    if (group == null) return;
    await attempt(() => api.stopSharing(groupId: group, file: file));
    await refreshRoom();
  }

  Future<void> download(String file) async {
    final group = roomState.roomGroup;
    final dir = prefs.downloadsDir;
    if (group == null || dir == null) return;
    // A folder that cannot be made (gone, read-only, a drive taken out) is said too.
    await attempt(() async {
      await Directory(dir).create(recursive: true);
      await api.download(groupId: group, file: file, dir: dir);
    });
    await refreshRoom();
  }

  /// Saves a picture of the room from its preview into the downloads folder, under a name
  /// not taken there yet, and says where.
  Future<void> savePicture(String fileId, String name) async {
    final bytes = roomState.previews[fileId];
    if (bytes == null) return;
    await _saveAs(
      (dir) => api.saveReceivedBytes(bytes: bytes, name: name, dir: dir),
    );
  }

  /// Where videos fetched to play are kept until the room is left: on disk, beside this
  /// device's data, since a long video would not fit in a temporary folder kept in memory.
  String get _videoCache => '$dataDir/videos';

  /// Where a video of the room shown can be played from: this device's own file, or a copy
  /// fetched from someone who has it, which goes when the room is left.
  Future<String?> videoToPlay(UiFile f) async {
    final path = f.path;
    if (path != null && f.downloading == null) return path;
    final group = roomState.roomGroup;
    if (group == null) return null;
    try {
      return await api.fetchToPlay(
        groupId: group,
        file: f.id,
        dir: _videoCache,
      );
    } catch (e) {
      // The room was left meanwhile: the fetch stopped with it, and nothing needs saying.
      if (roomState.roomGroup == group) say(explain(e), error: true);
      return null;
    }
  }

  /// Lets the videos fetched to play go.
  void _dropVideos() {
    try {
      final dir = Directory(_videoCache);
      if (dir.existsSync()) dir.deleteSync(recursive: true);
    } catch (_) {}
  }

  /// Saves a video fetched to play, or this device's own, into the downloads folder.
  Future<void> saveVideo(String path, String name) =>
      _saveAs((dir) => api.saveReceivedFile(path: path, name: name, dir: dir));

  /// Saves into the downloads folder through [save], which the core does: under the name
  /// the file was shared with, made safe there (a file that would act on its own when a
  /// file manager shows it gets an ending that makes it plain text), or a name not taken
  /// there yet. Says where it went.
  Future<void> _saveAs(Future<String> Function(String dir) save) async {
    final dir = prefs.downloadsDir;
    if (dir == null) return;
    try {
      final path = await save(dir);
      say(l10n.appSavedTo(path), path: path);
    } catch (e) {
      say(explain(e), error: true);
    }
  }

  // ----- Groups and friends -----

  Future<String?> createGroup(
    String name,
    int icon,
    List<String> friendIds,
  ) async {
    final id = await attempt(
      () => api.createGroup(name: name, icon: icon, friends: friendIds),
    );
    await refreshGroups();
    return id;
  }

  Future<void> refreshJoinAsks() async {
    try {
      contacts.joinAsks = await api.joinAsks();
    } catch (_) {}
    notices.soundForNew();
    notifyListeners();
  }

  /// Lets someone waiting with an invite into a group, or says no, which also cancels the
  /// invite they used.
  Future<void> answerJoin(UiJoinAsk ask, bool accept) async {
    contacts.joinAsks.removeWhere(
      (a) => a.deviceId == ask.deviceId && a.groupId == ask.groupId,
    );
    notifyListeners();
    await attempt(
      () => api.answerJoin(
        groupId: ask.groupId,
        deviceId: ask.deviceId,
        accept: accept,
      ),
    );
    await refreshJoinAsks();
  }

  /// The keys of every notice there is, the dismissed ones too, once every kind was read
  /// from the core; null until then.
  Iterable<String>? _noticeKeysNow() => notices.keysNow;

  /// Takes a notice out of the bell: a missed call is forgotten, in Friends too; the
  /// others stay dismissed in the next runs as well.
  void dismissNotice(Notice n) {
    final person = n.person?.id;
    if (n.kind == NoticeKind.missedCall && person != null) {
      if (notices.missedCalls[person] == n.atMs) {
        notices.missedCalls.remove(person);
      }
      if (notices.missedNote == person) notices.missedNote = null;
      if (notices.missedCalls.isEmpty) notices.missedUnseen = false;
      notices.changed();
      return;
    }
    // Said for this run only: once dismissed, it shows again only if said again.
    if (n.kind == NoticeKind.otherSession) {
      notices.otherSessionMs = null;
      notices.changed();
      return;
    }
    if (n.kind == NoticeKind.invitationDeclined) {
      notices.declines.removeWhere((d) => d.key == n.key);
      notices.changed();
      return;
    }
    // Messages are marked read rather than dismissed: the next ones show again.
    if (n.kind == NoticeKind.directMessage && person != null) {
      attempt(() => dm.markDirectRead(deviceId: person));
      return;
    }
    prefs.dismissedNotices.add(n.key);
    notices.changed();
    prefs.save();
  }

  void openGroupSettings(String group, {int tab = 0}) {
    nav.groupSettings = group;
    nav.groupSettingsTab = tab;
    nav.userPanelFrom = null;
    nav.changed();
  }

  void closeGroupSettings() {
    nav.groupSettings = null;
    nav.changed();
  }

  void showGroupSettingsTab(int tab) {
    nav.groupSettingsTab = tab;
    nav.changed();
  }

  /// The group's picture for everyone, or none to show its icon (managers).
  Future<bool> setGroupPicture(String group, Uint8List? picture) async {
    final ok = await succeeds(
      () => api.setGroupPicture(groupId: group, picture: picture),
    );
    await refreshGroups();
    return ok;
  }

  /// The group's banner for everyone, or none (managers).
  Future<bool> setGroupBanner(String group, Uint8List? picture) async {
    final ok = await succeeds(
      () => api.setGroupBanner(groupId: group, picture: picture),
    );
    await refreshGroups();
    return ok;
  }

  Future<bool> setGroupLimits(String group, int members, int room) async {
    final ok = await succeeds(
      () => api.setGroupLimits(groupId: group, members: members, room: room),
    );
    await refreshGroups();
    return ok;
  }

  /// Who manages [group]: the first [first] in it, or everyone (`null`).
  Future<bool> setGroupManagers(String group, int? first) async {
    final ok = await succeeds(
      () => api.setGroupManagers(groupId: group, first: first),
    );
    await refreshGroups();
    return ok;
  }

  /// Opens a voice room called `name`, or renames `voice` (managers).
  Future<bool> setVoiceRoom(String group, String? voice, String name) async {
    final ok = await succeeds(
      () => api.setVoiceRoom(groupId: group, voice: voice, name: name),
    );
    await refreshGroups();
    return ok;
  }

  Future<bool> removeVoiceRoom(String group, String voice) async {
    final ok = await succeeds(
      () => api.removeVoiceRoom(groupId: group, voice: voice),
    );
    await refreshGroups();
    return ok;
  }

  Future<bool> cancelInvites(String group) async {
    final ok = await succeeds(() => api.cancelInvites(groupId: group));
    await refreshGroups();
    return ok;
  }

  /// Adds a friend to a group; they choose whether to join (managers).
  Future<bool> inviteFriend(String group, String friend) async {
    final ok = await succeeds(
      () => api.addToGroup(groupId: group, friendId: friend),
    );
    if (ok) await refreshGroups();
    return ok;
  }

  /// Joins a group a friend added this device to, or declines it.
  Future<void> answerInvitation(String group, bool accept) async {
    await attempt(
      () => accept
          ? api.acceptGroupInvitation(groupId: group)
          : api.declineGroupInvitation(groupId: group),
    );
    await refreshGroups();
  }

  Future<void> answer(String device, bool accept) async {
    await attempt(() => api.answerRequest(deviceId: device, accept: accept));
    await refreshFriends();
  }

  Future<void> removeFriend(String device) async {
    await attempt(() => api.removeFriend(deviceId: device));
    await refreshFriends();
  }

  /// Uses a link or code someone gave: a friend request, or joining a group.
  Future<bool> useInvitation(String text) async {
    final t = text.trim();
    if (t.isEmpty) return false;
    if (t.startsWith('fogata:') && !t.startsWith('fogata:friend:')) {
      final name = await attempt(() => api.joinGroup(text: t));
      if (name == null) return false;
      say(l10n.appAskingToJoin(name));
      return true;
    }
    if (!await succeeds(() => api.addFriend(text: t))) return false;
    say(l10n.appLookingForDevice);
    await refreshFriends();
    return true;
  }

  /// Whether it was saved; if not, the person was told why.
  /// Changes this person's name and picture; [moving] is the animation that moves in
  /// place of the photo, whose first frame it is. The same photo keeps its animation.
  Future<bool> updateProfile(
    String name,
    int preset,
    List<int>? photo, {
    Uint8List? moving,
  }) async {
    final had = contacts.profile?.person.avatar.photo;
    if (photo == null) {
      moving = null;
    } else if (moving == null &&
        had != null &&
        samePicture(Uint8List.fromList(photo), had)) {
      moving = prefs.movingPhoto;
    }
    final saved = await succeeds(() async {
      await api.setProfile(
        name: name,
        preset: preset,
        photo: photo == null ? null : Uint8List.fromList(photo),
      );
      contacts.profile = await api.myProfile();
      if (!samePicture(moving, prefs.movingPhoto)) {
        await api.setAnimation(of: UiAnimated.photo, picture: moving);
        prefs.movingPhoto = moving;
        if (moving != null) {
          await writePrivate(prefs.movingPhotoFile.path, moving);
        } else if (await prefs.movingPhotoFile.exists()) {
          await prefs.movingPhotoFile.delete();
        }
      }
      final id = contacts.profile?.person.id;
      if (id != null) movingBook.forget(id);
    });
    // The new name or picture shows in the room and the groups' lists too.
    await Future.wait([refreshGroups(), refreshRoom()]);
    notifyListeners();
    return saved;
  }

  /// Changes the bio friends see on this device's card; an empty one shows none.
  Future<bool> updateBio(String text) async {
    final saved = await succeeds(() async {
      await api.setBio(text: text);
      contacts.bio = await api.myBio();
    });
    contacts.changed();
    return saved;
  }
}
