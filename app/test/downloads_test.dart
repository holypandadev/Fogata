// Where downloads go, and what is said when they cannot go there.

import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/rust/api/types.dart';

import 'fake_room.dart';

void main() {
  test('a downloads folder that cannot be made is said, not thrown', () async {
    final temp = await Directory.systemTemp.createTemp('fogata-downloads-');
    addTearDown(() => temp.delete(recursive: true));
    // A file where the folder would go: no folder can be made under it.
    final file = File('${temp.path}/taken')..writeAsStringSync('');
    final state = fakeRoom(people: 2)..prefs.downloadsDir = '${file.path}/sub';
    await state.download('f');
    expect(state.toasts.list.where((t) => t.error), hasLength(1));
  });

  test(
    'without a downloads folder from the system, one in the home folder',
    () {
      expect(
        downloadsFallback({'USERPROFILE': r'C:\Users\ana'}, windows: true),
        r'C:\Users\ana\Downloads',
      );
      expect(
        downloadsFallback({'HOME': '/home/ana'}, windows: false),
        '/home/ana/Downloads',
      );
      // Neither: where the app runs, the last resort.
      expect(downloadsFallback({}, windows: true), 'Downloads');
    },
  );

  test('the sounds kept for tests do not pile up', () {
    final state = AppState('/nonexistent/fogata-downloads-test');
    for (var i = 0; i < 1000; i++) {
      state.playMessageSound();
    }
    expect(state.played, isNotEmpty);
    expect(state.played.length, lessThanOrEqualTo(64));
    expect(state.played.last, UiSound.message);
  });
}
