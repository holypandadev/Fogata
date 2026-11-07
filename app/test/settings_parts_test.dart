import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/autostart.dart';
import 'package:fogata/src/call.dart' show ScreenQuality;
import 'package:fogata/src/l10n.dart';
import 'package:fogata/src/moving.dart' show maxMovingPhotoBytes;
import 'package:fogata/src/report.dart' show flushAppErrors;
import 'package:fogata/src/rust/api/direct.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/screens/settings.dart' show NotEncrypted;
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/theme.dart';
import 'package:fogata/src/widgets.dart';
import 'package:fogata/src/window.dart';

import 'fake_room.dart';

Future<AppState> _settings(
  WidgetTester tester,
  SettingsSection section, {
  Size size = const Size(1280, 800),
}) async {
  tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
    const MethodChannel('window_manager'),
    (call) async => false,
  );
  tester.view.physicalSize = size;
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  final state = fakeRoom(people: 2)
    ..callState.call = null
    ..roomState.room = null
    ..nav.page = Screen.home;
  await tester.pumpWidget(
    AppScope(
      state: state,
      child: MaterialApp(
        theme: Fg.darkTokens.themeData,
        home: const Material(child: Shell()),
      ),
    ),
  );
  state.openSettings();
  state.showSettingsSection(section);
  for (var i = 0; i < 4; i++) {
    await tester.pump(const Duration(milliseconds: 200));
  }
  // The settings ask the core for what a test does not have (devices, storage).
  tester.takeException();
  return state;
}

Future<void> _done(WidgetTester tester) async {
  await tester.pumpWidget(const SizedBox());
  await tester.pump(const Duration(seconds: 10));
}

Finder _labelled(String label) =>
    find.byWidgetPredicate((w) => w is Pressable && w.label == label);

Finder _nav(SettingsSection section) =>
    find.byKey(ValueKey('settings-nav:${section.name}'));

void main() {
  setUpAll(loadFonts);

  testWidgets('Language & time comes after Appearance and holds the time', (
    tester,
  ) async {
    final state = await _settings(tester, SettingsSection.appearance);
    final appearance = tester.getRect(_nav(SettingsSection.appearance));
    final language = tester.getRect(_nav(SettingsSection.languageTime));
    final voice = tester.getRect(_nav(SettingsSection.voice));
    expect(appearance.top, lessThan(language.top));
    expect(language.top, lessThan(voice.top));
    expect(find.text('Language & time'), findsOneWidget);
    // Appearance no longer has the time.
    expect(find.text('24 hours · 14:52'), findsNothing);
    state.showSettingsSection(SettingsSection.languageTime);
    await tester.pump(const Duration(milliseconds: 400));
    expect(find.text('24 hours · 14:52'), findsOneWidget);
    expect(find.text('Language'), findsOneWidget);
    await _done(tester);
  });

  testWidgets('a language picked in the list is the one Fogata speaks', (
    tester,
  ) async {
    final state = await _settings(tester, SettingsSection.languageTime);
    expect(state.prefs.language, 'en');
    // The list shows the language spoken now; picking another takes it.
    final list = find.byTooltip(l10n.settingsLanguageChoose);
    await tester.ensureVisible(list);
    await tester.pump(const Duration(milliseconds: 200));
    await tester.tap(list);
    // The menu comes in over a few frames, and takes no tap until it is in.
    for (var i = 0; i < 6; i++) {
      await tester.pump(const Duration(milliseconds: 100));
    }
    await tester.tap(find.text(languages['es']!).last);
    for (var i = 0; i < 6; i++) {
      await tester.pump(const Duration(milliseconds: 100));
    }
    tester.takeException();
    expect(state.prefs.language, 'es');
    expect(state.prefs.languageChosen, isTrue);
    await _done(tester);
  });

  test('the first language is the system\'s, when Fogata has it', () {
    expect(systemLanguage(const [Locale('de', 'AT')]), 'de');
    expect(systemLanguage(const [Locale('pt', 'PT')]), 'pt-BR');
    expect(systemLanguage(const [Locale('pt')]), 'pt-BR');
    expect(systemLanguage(const [Locale('zh', 'CN')]), 'zh-Hans');
    expect(systemLanguage(const [Locale('zh', 'SG')]), 'zh-Hans');
    expect(
      systemLanguage(const [
        Locale.fromSubtags(languageCode: 'zh', scriptCode: 'Hans'),
      ]),
      'zh-Hans',
    );
    // Traditional Chinese is not one: the next language the system lists is taken.
    expect(
      systemLanguage(const [Locale('zh', 'TW'), Locale('ja', 'JP')]),
      'ja',
    );
    expect(systemLanguage(const [Locale('zh', 'HK')]), 'en');
    expect(systemLanguage(const [Locale('nl', 'NL'), Locale('fr')]), 'fr');
    expect(systemLanguage(const [Locale('ko')]), 'en');
    expect(systemLanguage(const []), 'en');
  });

  test('app.json keeps the language only once it is chosen', () async {
    final dir = await Directory.systemTemp.createTemp('fogata-language');
    addTearDown(() => dir.delete(recursive: true));
    // Versions that offered only English wrote it as `language`: not a choice.
    File('${dir.path}/app.json').writeAsStringSync('{"language": "en"}');
    final prefs = Prefs(dir.path);
    prefs.language = 'ja';
    await prefs.load();
    expect(prefs.languageChosen, isFalse);
    expect(prefs.language, 'ja');
    File('${dir.path}/app.json').writeAsStringSync('{"uiLanguage": "xx"}');
    await prefs.load();
    expect(prefs.languageChosen, isFalse);
    File('${dir.path}/app.json').writeAsStringSync('{"uiLanguage": "de"}');
    await prefs.load();
    expect(prefs.languageChosen, isTrue);
    expect(prefs.language, 'de');
    prefs.language = 'en';
  });

  test(
    'a saved banner or animation larger than it was saved under is put aside',
    () async {
      final dir = await Directory.systemTemp.createTemp('fogata-saved');
      addTearDown(() => dir.delete(recursive: true));
      final banner = File('${dir.path}/banner.png');
      final moving = File('${dir.path}/banner.anim');
      final photo = File('${dir.path}/photo.anim');
      banner.writeAsBytesSync(Uint8List(Prefs.maxSavedBannerBytes + 1));
      moving.writeAsBytesSync(Uint8List(16));
      photo.writeAsBytesSync(Uint8List(maxMovingPhotoBytes + 1));
      final prefs = Prefs(dir.path);
      await prefs.load();
      await flushAppErrors();
      // Neither too large file is read, the start goes on, and both stay aside.
      expect(prefs.bannerPicture, isNull);
      expect(prefs.movingBanner, isNull);
      expect(prefs.movingPhoto, isNull);
      expect(File('${banner.path}.bad').existsSync(), isTrue);
      expect(File('${photo.path}.bad').existsSync(), isTrue);
      expect(banner.existsSync(), isFalse);
      // At the limit, they are read as before.
      banner.writeAsBytesSync(Uint8List(Prefs.maxSavedBannerBytes));
      photo.writeAsBytesSync(Uint8List(maxMovingPhotoBytes));
      final again = Prefs(dir.path);
      await again.load();
      expect(again.bannerPicture, hasLength(Prefs.maxSavedBannerBytes));
      expect(again.movingBanner, hasLength(16));
      expect(again.movingPhoto, hasLength(maxMovingPhotoBytes));
    },
  );

  test('app.json with a field of the wrong type keeps the others', () async {
    final dir = await Directory.systemTemp.createTemp('fogata-prefs');
    addTearDown(() => dir.delete(recursive: true));
    File('${dir.path}/app.json').writeAsStringSync(
      '{"volumes": "loud", "emoji": 5, "sounds": false, "uiLanguage": "de",'
      ' "silenced": ["a"]}',
    );
    final prefs = Prefs(dir.path);
    await prefs.load();
    await flushAppErrors();
    expect(prefs.sounds, isFalse);
    expect(prefs.silenced, {'a'});
    expect(prefs.language, 'de');
    expect(prefs.volumes, isEmpty);
    // Noted in the log, and a copy stays before the next save replaces it.
    expect(File('${dir.path}/app.json.bad').existsSync(), isTrue);
    expect(Directory('${dir.path}/diagnostics').existsSync(), isTrue);
    prefs.language = 'en';
  });

  test(
    'a setting that is on or off and damaged is said, not read as a choice',
    () async {
      final dir = await Directory.systemTemp.createTemp('fogata-prefs');
      addTearDown(() => dir.delete(recursive: true));
      File('${dir.path}/app.json')
          .writeAsStringSync('{"offline": "yes", "sounds": 0, "tray": false}');
      final prefs = Prefs(dir.path);
      await prefs.load();
      await flushAppErrors();
      expect(prefs.tray, isFalse);
      // Back to their defaults, and the file kept aside for the person to see.
      expect(prefs.appearOffline, isFalse);
      expect(prefs.sounds, isTrue);
      expect(prefs.setAsideMs, isNotNull);
      expect(File('${dir.path}/app.json.bad').existsSync(), isTrue);
    },
  );

  test(
    'a number, text or list of the wrong type is said too, not skipped',
    () async {
      Future<Prefs> read(String json) async {
        final dir = await Directory.systemTemp.createTemp('fogata-prefs');
        addTearDown(() => dir.delete(recursive: true));
        File('${dir.path}/app.json').writeAsStringSync(json);
        final prefs = Prefs(dir.path);
        await prefs.load();
        await flushAppErrors();
        expect(
          prefs.setAsideMs,
          isNotNull,
          reason: '$json is damaged and must be said',
        );
        expect(File('${dir.path}/app.json.bad').existsSync(), isTrue);
        return prefs;
      }

      final numbers = await read(
        '{"banner": "red", "noticesSeen": "1700000000000", "emojiTone": 2.5,'
        ' "tray": false}',
      );
      expect(numbers.tray, isFalse);
      expect(numbers.bannerColor, bannerColors.first.$1);
      expect(numbers.noticesSeenMs, 0);
      expect(numbers.emojiTone, 0);
      for (final json in [
        '{"theme": 1}',
        '{"audioInput": 7}',
        '{"screenQuality": "high"}',
        '{"screenQuality": {"lines": "720", "fps": 30}}',
        '{"uiLanguage": ["de"]}',
        '{"volumes": {"a": "loud"}}',
        '{"screenVolumes": {"a": 50.5}}',
        '{"silenced": "a"}',
        '{"screenSilenced": [1]}',
        '{"dismissed": [null]}',
        '{"emoji": ["x", 2]}',
      ]) {
        await read(json);
      }
      // A list with one damaged entry is not half taken.
      final lists = await read('{"silenced": ["a", 1], "emoji": ["x", 2]}');
      expect(lists.silenced, isEmpty);
      expect(lists.recentEmoji, isEmpty);
    },
  );

  test('settings of the right type that this version does not offer are '
      'not damage', () async {
    final dir = await Directory.systemTemp.createTemp('fogata-prefs');
    addTearDown(() => dir.delete(recursive: true));
    File('${dir.path}/app.json').writeAsStringSync(
      '{"theme": "sepia", "uiLanguage": "xx", "emojiTone": 9,'
      ' "screenQuality": {"lines": 480, "fps": 24}, "volumes": {"a": 900},'
      ' "audioInput": "default", "emoji": ["${'x' * 40}", "y"]}',
    );
    final prefs = Prefs(dir.path);
    await prefs.load();
    await flushAppErrors();
    expect(prefs.setAsideMs, isNull);
    expect(File('${dir.path}/app.json.bad').existsSync(), isFalse);
    expect(prefs.look, Look.dark);
    expect(prefs.languageChosen, isFalse);
    expect(prefs.emojiTone, 5);
    expect(prefs.screenQuality, const ScreenQuality());
    expect(prefs.volumes, {'a': 200});
    expect(prefs.audioInput, isNull);
    expect(prefs.recentEmoji, ['y']);
  });

  test('settings put aside are said once in the bell', () async {
    final dir = await Directory.systemTemp.createTemp('fogata-prefs');
    addTearDown(() => dir.delete(recursive: true));
    File('${dir.path}/app.json').writeAsStringSync('{"sounds": fal');
    final state = AppState(dir.path);
    await state.prefs.load();
    await flushAppErrors();
    state.warnAtStart();
    final notice = state.notices.list.single;
    expect(notice.warning!.title, 'Some settings were reset');
    expect(notice.warning!.text, contains('app.json.bad'));
    // Started again later in the same run (after a move): still once.
    state.warnAtStart();
    expect(state.notices.list, hasLength(1));
    state.dismissNotice(notice);
    expect(state.notices.list, isEmpty);
    // A start with nothing damaged says nothing.
    final fresh = await Directory.systemTemp.createTemp('fogata-prefs');
    addTearDown(() => fresh.delete(recursive: true));
    final clean = AppState(fresh.path);
    await clean.prefs.load();
    clean.warnAtStart();
    expect(clean.notices.list, isEmpty);
  });

  test('secrets kept without a keychain are said in the bell too', () async {
    final dir = await Directory.systemTemp.createTemp('fogata-prefs');
    addTearDown(() => dir.delete(recursive: true));
    final state = AppState(dir.path)..atRest = UiAtRest.noKeychain;
    await state.prefs.load();
    state.warnAtStart();
    final notice = state.notices.list.single;
    expect(notice.warning!.title, 'Not encrypted on this computer');
    expect(notice.warning!.text, contains('no system keychain'));
    // Dismissed, it stays dismissed at the next start too.
    state.dismissNotice(notice);
    state.warnAtStart();
    expect(state.notices.list, isEmpty);
    expect(state.prefs.dismissedNotices, contains('keychain'));
    // With one, nothing is said.
    final sealed = AppState(dir.path)..atRest = UiAtRest.sealed;
    sealed.warnAtStart();
    expect(sealed.notices.list, isEmpty);
  });

  test('secrets not all encrypted yet are said in the bell, apart', () async {
    final dir = await Directory.systemTemp.createTemp('fogata-prefs');
    addTearDown(() => dir.delete(recursive: true));
    // The notice for no keychain was dismissed earlier: this one still shows.
    final before = AppState(dir.path)..atRest = UiAtRest.noKeychain;
    await before.prefs.load();
    before.warnAtStart();
    before.dismissNotice(before.notices.list.single);
    final state = AppState(dir.path)..atRest = UiAtRest.unfinished;
    await state.prefs.load();
    state.warnAtStart();
    final notice = state.notices.list.single;
    expect(notice.warning!.title, 'Not all encrypted on this computer');
    expect(notice.warning!.text, contains('not encrypted yet'));
    // Once all of it is sealed, nothing is said.
    state.atRest = UiAtRest.sealed;
    state.warnAtStart();
    expect(state.notices.list, isEmpty);
  });

  test('app.json that is not a settings file is put aside', () async {
    final dir = await Directory.systemTemp.createTemp('fogata-prefs');
    addTearDown(() => dir.delete(recursive: true));
    File('${dir.path}/app.json').writeAsStringSync('{"sounds": fal');
    final prefs = Prefs(dir.path);
    await prefs.load();
    await flushAppErrors();
    expect(prefs.sounds, isTrue);
    expect(File('${dir.path}/app.json').existsSync(), isFalse);
    expect(
      File('${dir.path}/app.json.bad').readAsStringSync(),
      contains('fal'),
    );
    // No file at all is not damage.
    final fresh = await Directory.systemTemp.createTemp('fogata-prefs');
    addTearDown(() => fresh.delete(recursive: true));
    await Prefs(fresh.path).load();
    await flushAppErrors();
    expect(Directory('${fresh.path}/diagnostics').existsSync(), isFalse);
  });

  testWidgets(
    'Behaviour shows without a tray, and Open at startup starts off',
    (tester) async {
      // Tests have no tray icon.
      expect(hasTray, isFalse);
      final memory = MemoryAutostart();
      autostart = memory;
      final state = await _settings(tester, SettingsSection.behaviour);
      expect(_nav(SettingsSection.behaviour), findsOneWidget);
      expect(find.text('Open at startup'), findsOneWidget);
      expect(find.text('Keep running'), findsNothing);
      expect(_labelled('Open at startup off'), findsOneWidget);
      await tester.tap(_labelled('Open at startup off'));
      await tester.pump(const Duration(milliseconds: 300));
      expect(memory.writes, [true]);
      expect(memory.state, AutostartState.on);
      expect(_labelled('Open at startup on'), findsOneWidget);
      await tester.tap(_labelled('Open at startup on'));
      await tester.pump(const Duration(milliseconds: 300));
      expect(memory.writes, [true, false]);
      expect(memory.state, AutostartState.off);
      expect(state.toasts.list, isEmpty);
      await _done(tester);
    },
  );

  testWidgets(
    'data stored unencrypted for want of a keychain is said in Storage',
    (tester) async {
      for (final (atRest, said, unfinished) in [
        (UiAtRest.noKeychain, true, false),
        (UiAtRest.sealed, false, false),
        // Sealing began and did not end (a full disk, say).
        (UiAtRest.unfinished, false, true),
        // A copy run for tests, set to leave its data plain.
        (UiAtRest.plain, false, false),
        (null, false, false),
      ]) {
        final state = await _settings(tester, SettingsSection.storage);
        state.atRest = atRest;
        state.notifyListeners();
        await tester.pump(const Duration(milliseconds: 200));
        expect(
          find.text('Not encrypted on this computer'),
          said ? findsOneWidget : findsNothing,
          reason: '$atRest',
        );
        expect(
          find.text('Not all encrypted on this computer'),
          unfinished ? findsOneWidget : findsNothing,
          reason: '$atRest',
        );
        await _done(tester);
      }
    },
  );

  testWidgets('a system that cannot open Fogata at startup says so', (
    tester,
  ) async {
    final memory = MemoryAutostart(AutostartState.unsupported);
    autostart = memory;
    await _settings(tester, SettingsSection.behaviour);
    expect(
      find.text('This copy of Fogata cannot start automatically.'),
      findsOneWidget,
    );
    await tester.tap(_labelled('Open at startup off'));
    await tester.pump(const Duration(milliseconds: 300));
    expect(memory.writes, isEmpty);
    await _done(tester);
  });

  testWidgets(
    'waiting for macOS to allow it reads as on, with where to allow it',
    (tester) async {
      autostart = MemoryAutostart(AutostartState.needsApproval);
      await _settings(tester, SettingsSection.behaviour);
      expect(_labelled('Open at startup on'), findsOneWidget);
      expect(
        find.text('Allow Fogata in System Settings › General › Login Items.'),
        findsOneWidget,
      );
      await _done(tester);
    },
  );

  testWidgets('the storage switches say whether they are on', (tester) async {
    final state = await _settings(tester, SettingsSection.storage);
    state.direct.storage = const UiDirectStorage(
      total: UiDirectUsage(messages: 0, pictures: 0, videos: 0, files: 0),
      chats: [],
      settings: UiDirectSettings(
        limit: 0,
        autoPictures: true,
        autoFiles: false,
        timerMs: 0,
      ),
    );
    state.notifyListeners();
    await tester.pump(const Duration(milliseconds: 400));
    tester.takeException();
    expect(_labelled('Download pictures automatically on'), findsOneWidget);
    expect(
      _labelled('Download videos and files automatically off'),
      findsOneWidget,
    );
    await _done(tester);
  });

  testWidgets('without a tray, the network note does not send you to one', (
    tester,
  ) async {
    // Tests have no tray icon.
    await _settings(tester, SettingsSection.network);
    expect(
      find.text('The change applies once you quit Fogata and open it again.'),
      findsOneWidget,
    );
    expect(find.textContaining('tray icon'), findsNothing);
    await _done(tester);
  });

  testWidgets('a setting the system would not give says so', (tester) async {
    autostart = _Unreadable();
    addTearDown(() => autostart = MemoryAutostart());
    await _settings(tester, SettingsSection.behaviour);
    expect(
      find.text('Fogata could not read this setting from your system.'),
      findsOneWidget,
    );
    expect(
      tester.widget<Pressable>(_labelled('Open at startup off')).onTap,
      isNull,
    );
    await _done(tester);
  });

  test('a large app.json of many people heard keeps every setting; a huge one is put aside', () async {
    final dir = await Directory.systemTemp.createTemp('fogata-prefs');
    addTearDown(() => dir.delete(recursive: true));
    // About 2 MiB: a volume for each of 90,000 people, and the window kept in the tray.
    final volumes = {
      for (var i = 0; i < 90000; i++) i.toRadixString(16).padLeft(16, '0'): 50,
    };
    File('${dir.path}/app.json')
        .writeAsStringSync(jsonEncode({'tray': false, 'volumes': volumes}));
    expect(File('${dir.path}/app.json').lengthSync(), greaterThan(1024 * 1024));
    final prefs = Prefs(dir.path);
    await prefs.load();
    expect(prefs.setAsideMs, isNull);
    expect(prefs.tray, isFalse);
    // Past the most: kept aside for the person to see, and said.
    final huge = Directory('${dir.path}/huge')..createSync();
    File('${huge.path}/app.json').writeAsStringSync(
      '{"tray": false, "pad": "${'x' * (Prefs.maxSettingsBytes + 1)}"}',
    );
    final other = Prefs(huge.path);
    await other.load();
    expect(other.setAsideMs, isNotNull);
    expect(File('${huge.path}/app.json.bad').existsSync(), isTrue);
  });

  testWidgets('a damaged secret is named, with a way to set it aside', (
    tester,
  ) async {
    var setAside = 0;
    await tester.pumpWidget(
      MaterialApp(
        theme: Fg.darkTokens.themeData,
        home: Material(
          child: NotEncrypted(
            unfinished: true,
            damaged: 'friends.bin',
            onSetAside: () async => setAside++,
          ),
        ),
      ),
    );
    expect(find.textContaining('friends.bin'), findsOneWidget);
    await tester.tap(find.byKey(const ValueKey('set-damaged-aside')));
    await tester.pump();
    expect(setAside, 1);
    // Nothing damaged: neither said nor offered.
    await tester.pumpWidget(
      MaterialApp(
        theme: Fg.darkTokens.themeData,
        home: const Material(child: NotEncrypted(unfinished: true)),
      ),
    );
    expect(find.byKey(const ValueKey('set-damaged-aside')), findsNothing);
  });
}

/// A system whose autostart setting cannot be read.
class _Unreadable implements Autostart {
  @override
  Future<AutostartState> read() async => throw const FileSystemException('no');

  @override
  Future<AutostartState> write(bool on) async => AutostartState.off;
}
