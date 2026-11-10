import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/foundation.dart';

import 'announce.dart';
import 'app_state.dart';
import 'format.dart';
import 'l10n.dart';
import 'outgoing_send.dart';
import 'rust/api/app.dart' as api;
import 'rust/api/direct.dart' as dm;
import 'rust/api/types.dart';
import 'shown.dart';
import 'typing.dart';
import 'voice.dart';

/// The disappearing timers a chat can have, besides off: 24 hours, 7 days, 90 days.
const dayMs = 24 * 3600 * 1000;
const timers = [0, dayMs, 7 * dayMs, 90 * dayMs];

/// `Off`, `24 hours`, `7 days`, `90 days`.
String timerWord(int ms) => switch (ms) {
  0 => l10n.directStateTimerOff,
  dayMs => l10n.directStateTimer24h,
  _ => l10n.directStateTimerDays(ms ~/ dayMs),
};

/// When a message goes from this device, as hovering it tells: `Disappears in 5 min`,
/// `in 3 h`, `in 2 days`. [expiresMs] is when it goes, counted from when it reached
/// this device.
String disappearsIn(int expiresMs, {int? nowMs}) {
  const minute = 60 * 1000;
  final left = expiresMs - (nowMs ?? DateTime.now().millisecondsSinceEpoch);
  final minutes = (left / minute).round();
  if (minutes < 60) {
    return l10n.directStateDisappearsMinutes(minutes < 1 ? 1 : minutes);
  }
  final hours = (left / (60 * minute)).round();
  if (hours < 48) return l10n.directStateDisappearsHours(hours);
  return l10n.directStateDisappearsDays((left / dayMs).round());
}

/// The rooms for pictures, videos and files the settings offer, in bytes; 0 is no limit.
const storageLimits = [
  500000000,
  1000000000,
  2000000000,
  5000000000,
  10000000000,
  0,
];

/// `500 MB`, `2 GB`, `No limit`.
String limitWord(int bytes) => switch (bytes) {
  0 => l10n.directStateNoLimit,
  < 1000000000 => l10n.sizeMB('${bytes ~/ 1000000}'),
  _ => l10n.sizeGB('${bytes ~/ 1000000000}'),
};

/// Pictures up to this size come by themselves, when that is on.
const autoPictureBytes = 10 * 1024 * 1024;

/// Most messages pinned in a chat, as the core keeps them.
const maxPins = 50;

/// What the window keeps of the direct chats: the list, the chat shown with the files
/// waiting to go in it, and what the chats take on this computer. Everything else is the
/// core's, read again when it says something changed. Only what shows the chats hears
/// their changes.
class DirectChats extends Piece {
  DirectChats(this.app) {
    windowShown.addListener(shown);
  }

  final AppState app;

  /// The voice message being recorded and the one playing.
  late final voice = VoiceMessages(app);

  /// Tells what shows the chats that they changed.
  void _changed() => changed();

  /// The chats with something in them, newest first.
  List<dm.UiDirectChat> chats = [];

  /// The friend whose chat is shown, and its page.
  String? open;
  dm.UiDirectPage? page;

  /// How many of the newest items the page holds; more are read when scrolled to.
  int _limit = 200;

  @visibleForTesting
  int get limit => _limit;

  /// Files picked for the chat shown, by friend, until Send.
  final Map<String, List<OutgoingFile>> outgoing = {};

  /// Files of messages on their way, by friend: shown as sending until the message is in
  /// the chat, since a video is first read for its first picture and length.
  final Map<String, List<OutgoingFile>> sending = {};

  /// What was written in each chat and not sent yet, by friend, while the window is open:
  /// it is there again on coming back to the chat.
  final Map<String, String> drafts = {};

  /// Pictures of the chat shown read for the screen, by path (`direct.dart` reads and
  /// trims them): only that chat's, and only of files still in it, since some were
  /// sealed on disk and a message that disappeared should not stay in memory.
  final Map<String, Uint8List> pictures = {};

  /// What the chats take here, with the settings, once read.
  dm.UiDirectStorage? storage;

  /// Chats whose files were cleared in this window, for the settings to say so.
  final Set<String> cleared = {};

  /// The settings open at the storage part, once, when a chat asked to free space.
  bool storageOnOpen = false;

  /// The friends' banners for the chat's card, read again as the chat shows and when
  /// friends change, so one that arrives or changes later shows too.
  final banners = <String, UiBanner?>{};

  /// Friends whose banner was read since it last could have changed.
  final _bannersRead = <String>{};

  /// Friends changed, their banners among what may have: each is read again when its chat
  /// is next refreshed.
  void friendsChanged() => _bannersRead.clear();

  /// A message of the chat shown to bring into view, as a pinned one asks; a new value
  /// each time, so the same message can be asked for again.
  final jump = ValueNotifier<(String, int)?>(null);
  int _jumps = 0;

  final Map<String, Timer> _soon = {};
  int _lastSound = 0;

  /// When this window saw each chat's unread messages grow, by friend. A message carries
  /// its author's time, and the friend's clock may be behind this one's: the bell orders
  /// and counts new messages by when they came instead, or one could show as seen before.
  final Map<String, int> _cameMs = {};
  final Map<String, int> _unreadBefore = {};
  bool _listed = false;

  /// Takes the chats as the core lists them, noting which got new messages.
  void take(List<dm.UiDirectChat> fresh, {int? nowMs}) {
    final now = nowMs ?? DateTime.now().millisecondsSinceEpoch;
    for (final c in fresh) {
      final id = c.friend.id;
      if (_listed && c.unread > (_unreadBefore[id] ?? 0)) _cameMs[id] = now;
      _unreadBefore[id] = c.unread;
    }
    _listed = true;
    chats = fresh;
  }

  /// When the chat's newest unread messages came, as far as this window knows.
  int cameMs(dm.UiDirectChat c) =>
      math.max(c.lastMs, _cameMs[c.friend.id] ?? 0);

  /// Unread messages in every chat but the one being read.
  int get unread =>
      chats.fold(0, (n, c) => reading(c.friend.id) ? n : n + c.unread);

  /// The chat with `friend` in the list, if it has something in it.
  dm.UiDirectChat? chat(String? friend) {
    for (final c in chats) {
      if (c.friend.id == friend) return c;
    }
    return null;
  }

  /// Whether the chat with `friend` is the page shown, in a window that can be seen.
  bool reading(String? friend) =>
      friend != null &&
      open == friend &&
      app.nav.page == Screen.direct &&
      !app.nav.settingsOpen &&
      windowShown.value;

  /// Runs `work` soon, once, however many times it is asked for meanwhile.
  void _later(String key, Future<void> Function() work) {
    _soon[key]?.cancel();
    _soon[key] = Timer(const Duration(milliseconds: 120), () {
      _soon.remove(key);
      work();
    });
  }

  /// Reads the chats again, and the page shown unless [page] is false; its friend's banner
  /// only when it may have changed.
  Future<void> refresh({bool page = true}) async {
    try {
      take(await dm.directChats());
    } catch (_) {}
    if (open case final friend?) {
      if (page) await _loadPage();
      if (_bannersRead.add(friend)) await _loadBanner(friend);
    }
    _changed();
  }

  /// Reads the friend's banner, kept as it was when it did not change.
  Future<void> _loadBanner(String friend) async {
    UiBanner? banner;
    try {
      banner = await api.friendBanner(deviceId: friend);
    } catch (_) {
      _bannersRead.remove(friend);
      return;
    }
    final had = banners[friend];
    final same =
        banners.containsKey(friend) &&
        had?.color == banner?.color &&
        listEquals(had?.picture, banner?.picture);
    if (!same) banners[friend] = banner;
  }

  /// Reads the chat shown again; what came into it since is told to a screen reader
  /// unless [older] (older items read for scrolling up or for a message to show).
  Future<void> _loadPage({bool older = false}) async {
    final friend = open;
    if (friend == null) return;
    // Numbered, as the room's refreshes are: one asked earlier that answers later (a send
    // and an event at once, or a page read before more was asked for) is older news.
    final asked = ++_pageAsked;
    try {
      final loaded = await loadChat(deviceId: friend, limit: _limit);
      if (open != friend) return;
      if (asked < _pageShown) return;
      _pageShown = asked;
      if (!older) _announceNew(page, loaded);
      page = loaded;
      _keepPicturesOf(loaded);
      unawaited(voice.chatRead(friend, loaded));
    } catch (_) {
      return;
    }
    if (reading(friend) && (chat(friend)?.unread ?? 0) > 0) {
      try {
        await dm.markDirectRead(deviceId: friend);
      } catch (_) {}
    }
  }

  /// Lets go of the pictures read for files no longer in [shown]: gone with their
  /// message's timer, or deleted.
  void _keepPicturesOf(dm.UiDirectPage shown) {
    if (pictures.isEmpty) return;
    final paths = {
      for (final item in [...shown.items, ...shown.pinned])
        for (final f in item.files) ?f.path,
    };
    pictures.removeWhere((path, _) => !paths.contains(path));
  }

  int _pageAsked = 0, _pageShown = 0;

  /// Reads a chat's newest items; the tests put their own in its place.
  @visibleForTesting
  Future<dm.UiDirectPage> Function({
    required String deviceId,
    required int limit,
  })
  loadChat = dm.directChat;

  /// Tells a screen reader the friend's messages that came into the chat on screen
  /// since [before].
  void _announceNew(dm.UiDirectPage? before, dm.UiDirectPage after) {
    if (!reading(after.friend.id)) return;
    toldOf(newMessages(before, after)).forEach(announce);
  }

  /// Shows the chat with `friend`.
  Future<void> show(String friend) async {
    if (open != friend) {
      page = null;
      _limit = 200;
      pictures.clear();
    }
    _bannersRead.remove(friend);
    open = friend;
    app.nav.selectedGroup = null;
    app.nav.page = Screen.direct;
    app.nav.friendCard = null;
    app.nav.friendCardFrom = null;
    // The page changes too.
    app.nav.changed();
    _changed();
    await _loadPage();
    _changed();
    try {
      await dm.markDirectRead(deviceId: friend);
    } catch (_) {}
    // The page was just read.
    await refresh(page: false);
  }

  /// Reads older items of the chat shown; asked again while that is under way, the same
  /// reading answers, so scrolling asks for one page at a time.
  Future<void> more() {
    if (_more case final going?) return going;
    if (!(page?.more ?? false)) return Future.value();
    _limit += 200;
    return _more = () async {
      try {
        await _loadPage(older: true);
      } finally {
        _more = null;
      }
      _changed();
    }();
  }

  Future<void>? _more;

  /// Brings message `id` of the chat shown into view, reading older items until it is
  /// among them.
  Future<void> showMessage(String id) async {
    bool held() => page?.items.any((i) => i.id == id) ?? false;
    while (!held() && (page?.more ?? false)) {
      final before = page;
      await more();
      if (identical(page, before)) break;
    }
    if (!held()) {
      app.say(l10n.directStateMessageGone);
      return;
    }
    jump.value = (id, ++_jumps);
  }

  /// Pins message `id` of the chat shown for both people, or unpins it.
  Future<void> pin(String id, {required bool pinned}) async {
    final friend = open;
    if (friend == null) return;
    await app.attempt(
      () => dm.pinDirect(deviceId: friend, itemId: id, pinned: pinned),
    );
    await refresh();
  }

  /// Deletes message [id] of the open chat from this device; the friend keeps its copy.
  Future<void> deleteMessage(String id) async {
    final friend = open;
    if (friend == null) return;
    await app.attempt(
      () => dm.deleteDirectMessage(deviceId: friend, itemId: id),
    );
    await refresh();
  }

  /// Deletes this device's own message [id] of the open chat for both: here at once, on
  /// the friend's device when the deletion gets there.
  Future<void> unsendMessage(String id) async {
    final friend = open;
    if (friend == null) return;
    await app.attempt(() => dm.unsendDirect(deviceId: friend, itemId: id));
    await refresh();
  }

  List<OutgoingFile> get waiting => outgoing[open] ?? const [];

  List<OutgoingFile> get sendingHere => sending[open] ?? const [];

  /// Tells `friend`, if connected, that this device is typing in the chat with it, or
  /// stopped; nothing while it appears offline.
  void typing(String friend, bool on) {
    if (!app.started || app.prefs.appearOffline) return;
    dm.directTyping(deviceId: friend, typing: on).catchError((_) {});
  }

  /// Whether `friend` is typing in the chat with this device now.
  bool typingHere(String friend) => app.typing
      .who(TypingBoard.direct(friend), DateTime.now().millisecondsSinceEpoch)
      .contains(friend);

  /// Adds files to send in the chat shown. Folders cannot go and are left out, and said
  /// so; a file already waiting is not added twice.
  Future<void> addOutgoing(Iterable<String> paths) async {
    final friend = open;
    if (friend == null) return;
    final (files, folders) = await readOutgoing(paths);
    final list = outgoing.putIfAbsent(friend, () => []);
    for (final f in files) {
      if (!list.any((w) => w.path == f.path)) list.add(f);
    }
    if (list.isEmpty) outgoing.remove(friend);
    if (folders > 0) {
      app.say(l10n.directStateFoldersNotSent(folders));
    }
    _changed();
  }

  void removeOutgoing(OutgoingFile file) {
    final list = outgoing[open];
    list?.remove(file);
    if (list != null && list.isEmpty) outgoing.remove(open);
    _changed();
  }

  void clearOutgoing() {
    outgoing.remove(open);
    _changed();
  }

  /// Writes `text` with the files waiting, if any, to the friend shown; whether it went.
  /// When it did not, the files wait again and the text is kept as the chat's draft,
  /// unless something else was written meanwhile.
  Future<bool> send(String text) async {
    final friend = open;
    if (friend == null) return false;
    // At most what one message holds goes now; the rest waits for the next, and is not
    // read for its first picture before that.
    final waitingFiles = outgoing[friend] ?? const <OutgoingFile>[];
    final files = waitingFiles.take(maxOutgoingFiles).toList();
    final rest = waitingFiles.skip(maxOutgoingFiles).toList();
    if (text.trim().isEmpty && files.isEmpty) return false;
    if (rest.isEmpty) {
      outgoing.remove(friend);
    } else {
      outgoing[friend] = rest;
    }
    if (files.isNotEmpty) sending.putIfAbsent(friend, () => []).addAll(files);
    _changed();
    // The state was let go of before the message went: nothing was sent.
    var abandoned = false;
    final worked = await app.succeeds(() async {
      // The videos are read a few at a time, so as not to start a thread of the system's
      // for each.
      final going = await inPool(
        files,
        videoProbesAtOnce,
        _going,
        stop: () => app.isDisposed,
      );
      if (app.isDisposed) {
        abandoned = true;
        return;
      }
      await sendDirect(
        deviceId: friend,
        text: text.trim(),
        files: [for (final g in going) g!],
      );
    });
    final sent = worked && !abandoned;
    void gone() {
      final list = sending[friend];
      list?.removeWhere(files.contains);
      if (list != null && list.isEmpty) sending.remove(friend);
    }

    if (!sent) {
      gone();
      if (files.isNotEmpty) {
        final added = outgoing[friend] ?? const <OutgoingFile>[];
        outgoing[friend] = [
          ...files,
          ...added.where((a) => !files.any((f) => f.path == a.path)),
        ];
      }
      if (text.trim().isNotEmpty && (drafts[friend] ?? '').trim().isEmpty) {
        drafts[friend] = text;
      }
      _changed();
    }
    if (abandoned) return false;
    await refresh();
    // Sent: the files leave once the message is there to show them.
    if (sent && files.isNotEmpty) {
      gone();
      _changed();
    }
    return sent;
  }

  /// [f] as it goes: a video with its first picture and length.
  static Future<dm.UiDirectOutgoing> _going(OutgoingFile f) async =>
      dm.UiDirectOutgoing(path: f.path, video: await videoFor(f));

  /// Writes a direct message; the tests put their own in its place.
  @visibleForTesting
  Future<void> Function({
    required String deviceId,
    required String text,
    required List<dm.UiDirectOutgoing> files,
  })
  sendDirect = dm.sendDirect;

  /// Changes the disappearing timer of the chat shown, for both people.
  Future<void> setTimer(int ms) async {
    final friend = open;
    if (friend == null) return;
    await app.attempt(() => dm.setDirectTimer(deviceId: friend, timerMs: ms));
    await refresh();
  }

  /// Fetches an attachment of the chat shown from the friend.
  Future<void> fetch(String file) async {
    final friend = open;
    if (friend == null) return;
    await app.attempt(() => dm.fetchDirectFile(deviceId: friend, file: file));
    await refresh();
  }

  /// Copies an attachment kept here into the downloads folder, and says where.
  Future<void> save(String file) async {
    final friend = open;
    final dir = app.prefs.downloadsDir;
    if (friend == null || dir == null) return;
    final path = await app.attempt(
      () => dm.saveDirectFile(deviceId: friend, file: file, dir: dir),
    );
    if (path != null) app.say(l10n.appSavedTo(path), path: path);
  }

  /// Removes a chat's pictures, videos and files from this computer.
  Future<void> clearFiles(String friend) async {
    await app.attempt(() => dm.clearDirectFiles(deviceId: friend));
    if (open == friend) pictures.clear();
    cleared.add(friend);
    await Future.wait([refresh(), loadStorage()]);
  }

  /// Deletes a chat with someone who is no longer a friend, on this computer.
  Future<void> delete(String friend) async {
    await app.attempt(() => dm.deleteDirectChat(deviceId: friend));
    if (open == friend) pictures.clear();
    drafts.remove(friend);
    outgoing.remove(friend);
    if (open == friend) {
      open = null;
      page = null;
      app.goHome();
    }
    await refresh();
  }

  Future<void> loadStorage() async {
    try {
      storage = await dm.directStorage();
    } catch (_) {}
    _changed();
  }

  Future<void> setSettings(dm.UiDirectSettings settings) async {
    final before = storage;
    if (before != null) {
      storage = dm.UiDirectStorage(
        total: before.total,
        chats: before.chats,
        settings: settings,
      );
      _changed();
    }
    await app.attempt(() => dm.setDirectSettings(settings: settings));
    await Future.wait([loadStorage(), refresh()]);
  }

  void onEvent(UiEvent e) {
    switch (e.kind) {
      case UiEventKind.direct || UiEventKind.directDownload:
        _later('chats', refresh);
      case UiEventKind.directMessage:
        _later('chats', refresh);
        if (!reading(e.device)) _sound();
      case UiEventKind.directDownloadFailed:
        app.say(l10n.directStateFileFailed(explain(e.text ?? '')), error: true);
        _later('chats', refresh);
      default:
        break;
    }
  }

  /// The message sound, at most once every 0.8 s.
  void _sound() {
    final now = DateTime.now().millisecondsSinceEpoch;
    if (now - _lastSound < 800) return;
    _lastSound = now;
    app.playMessageSound();
  }

  /// Stops what waits to run, with the window.
  @override
  void dispose() {
    windowShown.removeListener(shown);
    voice.dispose();
    for (final t in _soon.values) {
      t.cancel();
    }
    _soon.clear();
    super.dispose();
  }

  /// The window came back into view: the chat shown is read now.
  void shown() {
    if (reading(open) && (chat(open)?.unread ?? 0) > 0) {
      _later('chats', refresh);
    }
  }
}

/// The friend's messages in [after] that came after the last item of [before], the same
/// chat read a moment earlier, as who wrote each and its words, or the files' names.
/// Older items read for scrolling up are not new; nor is anything when the chat was not
/// read before, as opening a chat is not a message coming.
List<(String, String)> newMessages(
  dm.UiDirectPage? before,
  dm.UiDirectPage after,
) {
  if (before == null || before.friend.id != after.friend.id) return const [];
  final had = {for (final item in before.items) item.id};
  final last = before.items.lastOrNull;
  // The items come oldest first: what follows the last one known is new.
  final at = last == null
      ? -1
      : after.items.indexWhere((item) => item.id == last.id);
  return [
    for (final (i, item) in after.items.indexed)
      if (!item.mine &&
          !had.contains(item.id) &&
          (last == null || (at >= 0 ? i > at : item.atMs > last.atMs)))
        (
          after.friend.name,
          item.text.isNotEmpty
              ? item.text
              : item.files.map((f) => f.name).join(', '),
        ),
  ];
}
