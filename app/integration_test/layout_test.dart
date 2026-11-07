// Pictures of a call's layout with made-up participants: window sizes from the smallest
// Fogata allows up to a large screen, from two people to more than a room takes, and with
// screens shared and watched. Nothing connects; no microphone, camera or screen is used.
//
//   FOGATA_SHOTS=<folder> flutter test integration_test/layout_test.dart -d linux
//
// Under GDK_SCALE=2 the pictures have twice the pixels, as on a Retina screen, which shows
// how sharp the pictures of people are drawn.

import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/photo.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/theme.dart';
import 'package:integration_test/integration_test.dart';

import '../test/fake_room.dart';

final _shots = Platform.environment['FOGATA_SHOTS'];

Future<void> _wait(WidgetTester tester, Duration d) async {
  final end = DateTime.now().add(d);
  while (DateTime.now().isBefore(end)) {
    await Future<void>.delayed(const Duration(milliseconds: 50));
    await tester.pump();
  }
}

/// Saves a picture of the window at its own pixel ratio.
Future<void> _shot(WidgetTester tester, String name) async {
  if (_shots == null) return;
  final view = tester.binding.renderViews.first;
  final layer = view.debugLayer! as OffsetLayer;
  // The root layer already scales by the pixel ratio.
  final image = await layer.toImage(
    Offset.zero & (view.size * tester.view.devicePixelRatio),
  );
  final data = await image.toByteData(format: ui.ImageByteFormat.png);
  await File('$_shots/$name.png').writeAsBytes(data!.buffer.asUint8List());
}

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('call layouts', (tester) async {
    // The window's own ratio: 2 under GDK_SCALE=2.
    final ratio = tester.view.devicePixelRatio;
    final welcome = await rootBundle.load('assets/images/welcome.png');
    // A picture made the way a profile picture is made from a chosen file.
    final photo = await squarePicture(welcome.buffer.asUint8List());
    final scenarios = <(String, AppState Function(), List<Size>)>[
      for (final people in [2, 3, 4, 6, 8])
        (
          'people-$people',
          () => fakeRoom(people: people),
          const [Size(960, 640), Size(1440, 900), Size(1920, 1080)],
        ),
      ('people-12', () => fakeRoom(people: 12), const [Size(960, 640)]),
      (
        'shared-all',
        () => fakeRoom(people: 8, sharing: 7, own: true),
        const [Size(960, 640), Size(1920, 1080)],
      ),
      (
        'watching-many',
        () => fakeRoom(people: 8, sharing: 7, own: true, watching: true),
        const [
          Size(960, 640),
          Size(1280, 800),
          Size(1920, 1080),
          Size(2560, 1440),
        ],
      ),
      (
        'watching-few',
        () => fakeRoom(people: 3, sharing: 1, watching: true),
        const [Size(960, 640), Size(1440, 900), Size(1920, 1080)],
      ),
      (
        'own-screen',
        () => fakeRoom(people: 4, own: true, ownShown: true),
        const [Size(1280, 800)],
      ),
      (
        'photos',
        () => fakeRoom(people: 4, photo: photo),
        const [Size(1280, 800)],
      ),
    ];
    final problems = <String>[];
    for (final (name, make, sizes) in scenarios) {
      for (final size in sizes) {
        final state = make();
        tester.view.physicalSize = size * ratio;
        tester.view.devicePixelRatio = ratio;
        await tester.pumpWidget(
          AppScope(
            state: state,
            child: MaterialApp(
              theme: Fg.darkTokens.themeData,
              home: const Material(child: Shell()),
            ),
          ),
        );
        await _wait(tester, const Duration(milliseconds: 700));
        final e = tester.takeException();
        if (e != null) problems.add('$name at $size: $e');
        await _shot(
          tester,
          '$name-${size.width.round()}x${size.height.round()}'
          '${ratio == 1 ? '' : '@${ratio}x'}',
        );
        // Never started, so nothing to stop.
        state.callState.call = null;
      }
    }
    // The stream quality, opened from beside the share button and from over the screen.
    final state = fakeRoom(people: 4, own: true, ownShown: true);
    tester.view.physicalSize = const Size(1280, 800) * ratio;
    await tester.pumpWidget(
      AppScope(
        state: state,
        child: MaterialApp(
          theme: Fg.darkTokens.themeData,
          home: const Material(child: Shell()),
        ),
      ),
    );
    await _wait(tester, const Duration(milliseconds: 500));
    for (final (i, name) in ['screen', 'dock'].indexed) {
      await tester.tap(find.byTooltip('Stream quality').at(i));
      await _wait(tester, const Duration(milliseconds: 600));
      await _shot(
        tester,
        'stream-quality-$name${ratio == 1 ? '' : '@${ratio}x'}',
      );
      await tester.tapAt(const Offset(4, 400));
      await _wait(tester, const Duration(milliseconds: 400));
    }
    final e = tester.takeException();
    if (e != null) problems.add('stream quality: $e');
    state.callState.call = null;
    await tester.pumpWidget(const SizedBox());

    // A watched screen with the chat beside it, and with the chat hidden: by hand on a
    // wide window, from the start on a small one (shown by hand there).
    final suffix = ratio == 1 ? '' : '@${ratio}x';
    for (final size in const [Size(960, 640), Size(1280, 800)]) {
      final narrow = fakeRoom(people: 3, sharing: 1, watching: true);
      // The window follows the call's changes, as it does once a call starts.
      narrow.call!.addListener(() => narrow.go(narrow.nav.page));
      tester.view.physicalSize = size * ratio;
      await tester.pumpWidget(
        AppScope(
          state: narrow,
          child: MaterialApp(
            theme: Fg.darkTokens.themeData,
            home: const Material(child: Shell()),
          ),
        ),
      );
      await _wait(tester, const Duration(milliseconds: 700));
      final at = '${size.width.round()}x${size.height.round()}$suffix';
      final shown = narrow.roomChatShown;
      await _shot(tester, '${shown ? 'chat-shown' : 'chat-hidden'}-$at');
      await tester.tap(
        find.byTooltip(shown ? 'Hide the chat' : 'Show the chat'),
      );
      await _wait(tester, const Duration(milliseconds: 700));
      await _shot(tester, '${shown ? 'chat-hidden' : 'chat-shown'}-$at');
      final e = tester.takeException();
      if (e != null) problems.add('chat at $size: $e');
      narrow.callState.call = null;
      await tester.pumpWidget(const SizedBox());
    }

    // The watched screen floats over other pages: in the corner, dragged, and in the
    // light theme.
    for (final (theme, name) in [
      (Fg.darkTokens, 'dark'),
      (Fg.lightTokens, 'light'),
    ]) {
      final watching = fakeRoom(people: 3, sharing: 1, watching: true);
      watching.call!.addListener(() => watching.go(watching.nav.page));
      tester.view.physicalSize = const Size(1280, 800) * ratio;
      await tester.pumpWidget(
        AppScope(
          state: watching,
          child: MaterialApp(
            theme: theme.themeData,
            home: const Material(child: Shell()),
          ),
        ),
      );
      await _wait(tester, const Duration(milliseconds: 500));
      watching.goHome();
      await _wait(tester, const Duration(milliseconds: 900));
      await _shot(tester, 'pip-$name-home$suffix');
      final pip = find.byKey(const ValueKey('pip'));
      if (name == 'dark') {
        watching.say('Profile saved. Friends and groups online see it now.');
        await _wait(tester, const Duration(milliseconds: 700));
        await _shot(tester, 'pip-dark-toast$suffix');
        final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
        await mouse.addPointer(location: tester.getCenter(pip));
        await mouse.down(tester.getCenter(pip));
        await mouse.moveBy(const Offset(-200, -150));
        await _wait(tester, const Duration(milliseconds: 100));
        await mouse.moveBy(const Offset(-200, -150));
        await _wait(tester, const Duration(milliseconds: 500));
        await _shot(tester, 'pip-dark-dragging$suffix');
        await mouse.up();
        await _wait(tester, const Duration(milliseconds: 700));
        await _shot(tester, 'pip-dark-dropped$suffix');
        await mouse.removePointer();
        tester.view.physicalSize = const Size(960, 640) * ratio;
        await _wait(tester, const Duration(milliseconds: 500));
        await _shot(tester, 'pip-dark-smaller$suffix');
      } else {
        watching.go(Screen.friends);
        await _wait(tester, const Duration(milliseconds: 900));
        await _shot(tester, 'pip-light-friends$suffix');
      }
      // The pages ask the core for what a test does not have, and say so.
      tester.takeException();
      watching.callState.call = null;
      await tester.pumpWidget(const SizedBox());
      await _wait(tester, const Duration(seconds: 8));
    }
    expect(problems, isEmpty);
  });
}
