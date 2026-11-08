import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/rust/api/types.dart';

import 'wait.dart';

const ana = UiPerson(
  id: 'ana',
  name: 'Ana',
  short: '7Q4M·K2XR',
  avatar: UiAvatar(preset: 1),
);

UiRoom room(String path, String name, int size) => UiRoom(
  startedMs: 0,
  joined: true,
  participants: const [],
  chat: const [],
  files: [
    UiFile(
      sharedMs: 0,
      id: 'f',
      name: name,
      size: size,
      sharedBy: ana,
      holders: const [ana],
      mine: true,
      sent: 0,
      path: path,
    ),
  ],
);

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test(
    'a damaged picture of the room shows as a file, not as one on its way',
    () async {
      final dir = Directory.systemTemp.createTempSync('fogata-previews');
      // It starts like a PNG and is not one.
      final path = '${dir.path}/beach.png';
      File(path).writeAsBytesSync([
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 1, 2, 3, //
      ]);
      final state = AppState('/nonexistent/fogata-previews-test')
        ..roomState.roomGroup = 'g';
      final r = room(path, 'beach.png', 11);
      state.roomState.fetchPreviews('g', r);
      await until(
        () => state.roomState.noPreview.contains('f'),
        what: 'the picture shown as a file',
      );
      expect(state.roomState.noPreview, contains('f'));
      expect(state.roomState.previews, isEmpty);
      dir.deleteSync(recursive: true);
    },
  );

  test('a picture that cannot be fetched shows as a file after a few tries', () async {
    final state = AppState('/nonexistent/fogata-previews-test')
      ..roomState.roomGroup = 'g';
    // Gone from where it was shared from: every try fails.
    final r = room(
      '/nonexistent/fogata-previews-test/lake.png',
      'lake.png',
      11,
    );
    for (var i = 0; i < 3; i++) {
      expect(state.roomState.noPreview, isEmpty);
      state.roomState.fetchPreviews('g', r);
      // Each try ends before the next: one still going would be left as it is.
      await until(
        () => !state.roomState.previewing('f'),
        what: 'try ${i + 1} ended',
      );
    }
    await until(
      () => state.roomState.noPreview.contains('f'),
      what: 'the picture shown as a file',
    );
    expect(state.roomState.noPreview, contains('f'));
  });
}
