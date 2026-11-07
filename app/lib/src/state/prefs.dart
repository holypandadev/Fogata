import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:flutter/material.dart';

import '../call.dart';
import '../format.dart' as format;
import '../l10n.dart';
import '../moving.dart' show maxMovingBannerBytes, maxMovingPhotoBytes;
import '../photo.dart' show readAtMost;
import '../report.dart' show noteAppError;
import '../rust/api/app.dart' as api;
import '../theme.dart';
import 'piece.dart';

/// How many emoji the picker keeps as recently used.
const maxRecentEmoji = 32;

/// What the person chose on this computer, kept in `app.json` in the data folder, with
/// their banner's picture and their animations kept beside it.
class Prefs extends Piece {
  Prefs(this.dataDir) {
    language = systemLanguage();
  }

  final String dataDir;

  /// The window's theme.
  Look look = Look.dark;
  ThemeMode get themeMode => look.mode;
  bool sounds = true;

  /// Whether closing the window leaves Fogata running in the tray rather than quitting.
  bool tray = true;

  /// Clock times in 12 hours with AM and PM, or in 24.
  bool get clock12 => format.clock12;
  set clock12(bool on) => format.clock12 = on;

  /// Whether this device carries other members' traffic when they cannot connect directly.
  /// Read by the core at start: a change applies at the next start.
  bool bridge = true;

  /// Whether this device appears offline: friends and members see it as offline, except
  /// in a room it is in, and nothing is disconnected. The core reads it at start too.
  bool appearOffline = false;

  /// The top of this person's panel: a colour, and a picture over it if one was chosen.
  /// Friends see them on this person's card, the picture made small enough to send.
  Color bannerColor = bannerColors.first.$1;
  Uint8List? bannerPicture;

  /// The animations of this person's picture and banner, when they move (moving.dart).
  Uint8List? movingPhoto;
  Uint8List? movingBanner;

  /// The chat and files are shown beside a room or call on a wide window until hidden;
  /// remembered.
  bool roomChat = true;

  /// How to enter the next room with the microphone: it stays as it was left in the last
  /// call.
  bool joinWithMic = true;

  /// Chosen devices (WebRTC device ids); null for the system's default.
  String? audioInput;
  String? audioOutput;
  String? videoInput;

  /// This device's own camera is drawn mirrored, as in a mirror, wherever it shows here
  /// (the settings' preview, the room's tile, the enlarged picture). Only how it is drawn
  /// on this computer: what the others get is the camera's picture as it is.
  bool mirrorCamera = true;

  /// The microphone's steady background noise is lowered in calls. Off until turned on.
  bool noiseSuppression = false;

  /// How this device's shared screen goes out, kept for the next calls.
  ScreenQuality screenQuality = const ScreenQuality();

  /// Where downloads and saved pictures go, found at start.
  String? downloadsDir;

  /// How loud each person's voice plays on this device, in percent from 0 to 200, by
  /// device id; 100 when missing. Kept for the next calls.
  final Map<String, int> volumes = {};

  /// People whose voice this device does not play at all, whatever their volume.
  final Set<String> silenced = {};

  /// The same for the sound of each person's shared screen.
  final Map<String, int> screenVolumes = {};
  final Set<String> screenSilenced = {};

  /// Notices dismissed from the bell, by key, and when the bell was last opened.
  final Set<String> dismissedNotices = {};
  int noticesSeenMs = 0;

  /// The emoji picked last, newest first, in their default form. Picking one tells
  /// nobody: a picker open keeps its order until it opens again.
  List<String> recentEmoji = [];

  /// The skin tone of the emoji picked, 0 (default) to 5.
  int emojiTone = 0;

  /// The interface's language, one of [languages]: the one chosen in Settings, or until
  /// then the system's. Setting it says every word in it from then on.
  String get language => _language;
  set language(String code) {
    useLanguage(code);
    _language = currentLanguage;
  }

  String _language = 'en';

  /// Whether the person chose [language] in Settings, rather than it following the
  /// system's.
  bool languageChosen = false;

  int volumeOf(String id) => volumes[id] ?? 100;
  int screenVolumeOf(String id) => screenVolumes[id] ?? 100;

  /// The keys of the bell's notices there are now, once every kind was read from the core;
  /// null until then. Dismissed notices that are gone (a request answered, an invitation
  /// taken) are not kept, or they would stay in the file for good.
  Iterable<String>? Function()? noticesNow;

  File get _prefsFile => File('$dataDir/app.json');

  /// Most bytes of the banner's picture kept here: [bannerPng] makes one at most 900 by
  /// 300 pixels, which fits in this as a PNG with room to spare.
  static const maxSavedBannerBytes = 2 * 1024 * 1024;

  /// Most bytes of `app.json`: what it keeps takes a few kilobytes, and grows with the
  /// people heard (a volume, a silence, each): years of them stay well within this, which
  /// still keeps a file made huge from filling the memory. One past it is put aside whole.
  static const maxSettingsBytes = 8 * 1024 * 1024;

  File get bannerFile => File('$dataDir/banner.png');
  File get movingPhotoFile => File('$dataDir/photo.anim');
  File get movingBannerFile => File('$dataDir/banner.anim');

  /// Whether [load] ran.
  bool loaded = false;

  /// When [load] put `app.json` aside, whole or as a copy, because something in it could
  /// not be read: the person is told once.
  int? setAsideMs;

  /// Reads what was kept: `app.json`, the banner's picture and the animations. A field of
  /// `app.json` that cannot be read keeps its default and the others still load; a file that
  /// is not a settings file at all is put aside as `app.json.bad`. Both are noted in the log,
  /// so the next save does not silently replace what the person had chosen.
  Future<void> load() async {
    loaded = true;
    String? text;
    final bytes = await _readSaved(
      _prefsFile,
      maxSettingsBytes,
      putAside: () => setAsideMs = DateTime.now().millisecondsSinceEpoch,
    );
    if (bytes != null) text = utf8.decode(bytes, allowMalformed: true);
    if (text != null) _read(text);
    bannerPicture = await _readSaved(bannerFile, maxSavedBannerBytes);
    if (bannerPicture != null) {
      movingBanner = await _readSaved(movingBannerFile, maxMovingBannerBytes);
    }
    movingPhoto = await _readSaved(movingPhotoFile, maxMovingPhotoBytes);
  }

  /// A picture or animation kept here, read with the limit it was saved under: one
  /// larger than that (changed outside Fogata, or damaged) is not read whole into memory
  /// but put aside as `<name>.bad` and noted in the log, and the start goes on without it.
  Future<Uint8List?> _readSaved(
    File file,
    int max, {
    void Function()? putAside,
  }) async {
    Uint8List? bytes;
    try {
      if (await file.length() <= max) {
        bytes = await readAtMost(file.openRead(0, max + 1), max);
      }
    } on PathNotFoundException {
      return null;
    } catch (e) {
      noteAppError(
        dataDir,
        '${file.uri.pathSegments.last} cannot be read: $e',
        null,
      );
      return null;
    }
    if (bytes != null) return bytes;
    noteAppError(
      dataDir,
      '${file.uri.pathSegments.last} is larger than $max bytes: put aside',
      null,
    );
    try {
      await file.rename('${file.path}.bad');
      putAside?.call();
    } catch (_) {}
    return null;
  }

  /// Takes what `app.json` holds, field by field.
  void _read(String text) {
    final Map<String, dynamic> prefs;
    try {
      prefs = jsonDecode(text) as Map<String, dynamic>;
    } catch (e) {
      noteAppError(dataDir, 'app.json is not a settings file: $e', null);
      try {
        _prefsFile.renameSync('${_prefsFile.path}.bad');
        setAsideMs = DateTime.now().millisecondsSinceEpoch;
      } catch (_) {}
      return;
    }
    var failed = false;
    void field(String name, void Function() take) {
      try {
        take();
      } catch (e) {
        failed = true;
        noteAppError(dataDir, 'app.json: "$name" ignored: $e', null);
      }
    }

    // A setting that is on or off is true, false or missing: anything else is damage, not
    // a choice, and is said like any other field that cannot be read. So is a number, text,
    // list or map of the wrong type; a value of the right type that this version does not
    // offer (a theme or language it lacks, a tone out of range) is not damage.
    bool flag(String name, bool missing) => switch (prefs[name]) {
      null => missing,
      final bool on => on,
      final other => throw FormatException('not true or false: $other'),
    };
    T? typed<T>(Object? value) => switch (value) {
      null => null,
      final T v => v,
      final other => throw FormatException('not a $T: $other'),
    };
    // Lists and maps are read whole before anything is taken, so a damaged entry does not
    // leave half of one.
    List<T> listOf<T>(String name) => [
      for (final e in typed<List>(prefs[name]) ?? const []) typed<T>(e) as T,
    ];
    Map<String, T> mapOf<T>(String name) => {
      for (final MapEntry(:key, :value)
          in (typed<Map>(prefs[name]) ?? const {}).entries)
        typed<String>(key)!: typed<T>(value) as T,
    };
    field('theme', () => look = Look.named(typed<String>(prefs['theme'])));
    field('sounds', () => sounds = flag('sounds', true));
    field('tray', () => tray = flag('tray', true));
    field('clock12', () => clock12 = flag('clock12', false));
    field('roomChat', () => roomChat = flag('roomChat', true));
    field('bridge', () => bridge = flag('bridge', true));
    field('offline', () => appearOffline = flag('offline', false));
    field('banner', () {
      if (typed<int>(prefs['banner']) case final banner?) {
        bannerColor = Color(banner);
      }
    });
    field('mic', () => joinWithMic = flag('mic', true));
    String? device(String key) => switch (typed<String>(prefs[key])) {
      final String id when !defaultAlias(id) => id,
      _ => null,
    };
    field('audioInput', () => audioInput = device('audioInput'));
    field('audioOutput', () => audioOutput = device('audioOutput'));
    field('videoInput', () => videoInput = device('videoInput'));
    field('mirrorCamera', () => mirrorCamera = flag('mirrorCamera', true));
    field(
      'noiseSuppression',
      () => noiseSuppression = flag('noiseSuppression', false),
    );
    field('screenQuality', () {
      final quality = typed<Map>(prefs['screenQuality']);
      typed<int>(quality?['lines']);
      typed<int>(quality?['fps']);
      screenQuality = ScreenQuality.fromJson(quality);
    });
    field('volumes', () {
      for (final MapEntry(:key, :value) in mapOf<int>('volumes').entries) {
        volumes[key] = value.clamp(0, 200);
      }
    });
    field('silenced', () => silenced.addAll(listOf<String>('silenced')));
    field('screenVolumes', () {
      for (final MapEntry(:key, :value) in mapOf<int>(
        'screenVolumes',
      ).entries) {
        screenVolumes[key] = value.clamp(0, 200);
      }
    });
    field(
      'screenSilenced',
      () => screenSilenced.addAll(listOf<String>('screenSilenced')),
    );
    field(
      'dismissed',
      () => dismissedNotices.addAll(listOf<String>('dismissed')),
    );
    field('noticesSeen', () {
      if (typed<int>(prefs['noticesSeen']) case final seen?) {
        noticesSeenMs = seen;
      }
    });
    field('emoji', () {
      recentEmoji = [
        for (final e in listOf<String>('emoji'))
          if (e.length <= 32) e,
      ].take(maxRecentEmoji).toList();
    });
    field('emojiTone', () {
      if (typed<int>(prefs['emojiTone']) case final tone?) {
        emojiTone = tone.clamp(0, 5);
      }
    });
    field('uiLanguage', () {
      // Kept as `uiLanguage`: `language` holds "en" from versions that offered no other.
      if (typed<String>(prefs['uiLanguage']) case final code?
          when languages.containsKey(code)) {
        language = code;
        languageChosen = true;
      }
    });
    // What could not be read is replaced by the next save: a copy stays beside the file.
    if (failed) {
      try {
        _prefsFile.copySync('${_prefsFile.path}.bad');
        setAsideMs = DateTime.now().millisecondsSinceEpoch;
      } catch (_) {}
    }
  }

  /// The settings file's last write: each waits for the one before, so the newest
  /// settings are the ones the file keeps.
  Future<void> _written = Future.value();

  /// Writes `app.json` with what is chosen now.
  Future<void> save() async {
    try {
      if (noticesNow?.call() case final keys?) dismissedNotices.retainAll(keys);
      final json = jsonEncode({
        'theme': look.name,
        'sounds': sounds,
        'tray': tray,
        'clock12': clock12,
        'roomChat': roomChat,
        'bridge': bridge,
        'offline': appearOffline,
        'banner': bannerColor.toARGB32(),
        'mic': joinWithMic,
        'audioInput': ?audioInput,
        'audioOutput': ?audioOutput,
        'videoInput': ?videoInput,
        'mirrorCamera': mirrorCamera,
        'noiseSuppression': noiseSuppression,
        'screenQuality': screenQuality.toJson(),
        'volumes': volumes,
        'silenced': silenced.toList(),
        'screenVolumes': screenVolumes,
        'screenSilenced': screenSilenced.toList(),
        'dismissed': dismissedNotices.toList(),
        'noticesSeen': noticesSeenMs,
        'emoji': recentEmoji,
        'emojiTone': emojiTone,
        'uiLanguage': ?(languageChosen ? language : null),
      });
      final written = _written.then((_) async {
        await Directory(dataDir).create(recursive: true);
        // Written by the core, so it gets the same private permissions as the other files.
        await api.writePrefs(dataDir: dataDir, json: json);
      });
      _written = written.catchError((_) {});
      await written;
    } catch (_) {}
  }
}
