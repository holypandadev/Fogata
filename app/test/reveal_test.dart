import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/reveal.dart';

void main() {
  test('a file is shown in its folder by one address, commas and all', () {
    final item = fileManagerItem('/home/ana/Downloads/a,b c.txt');
    expect(item, isNot(contains(',')));
    expect(
      Uri.parse(item).toFilePath(windows: false),
      '/home/ana/Downloads/a,b c.txt',
    );
  });

  test('without dbus-send, or when it fails, the folder opens', () async {
    for (final missing in [true, false]) {
      final ran = <String>[];
      final shown = await showInFolderFreedesktop(
        '/home/ana/Downloads/plan.txt',
        run: (command, args) async {
          ran.add(command);
          if (command == 'dbus-send' && missing) {
            throw const ProcessException('dbus-send', [], 'not found', 2);
          }
          return ProcessResult(0, command == 'dbus-send' ? 1 : 0, '', '');
        },
      );
      expect(ran, ['dbus-send', 'xdg-open']);
      expect(shown, isTrue);
    }
    final ran = <String>[];
    final shown = await showInFolderFreedesktop(
      '/home/ana/Downloads/plan.txt',
      run: (command, args) async {
        ran.add(command);
        return ProcessResult(0, 0, '', '');
      },
    );
    expect(ran, ['dbus-send'], reason: 'the file manager showed it');
    expect(shown, isTrue);
  });

  test('when nothing can show the folder, it says so', () async {
    for (final xdgOpen in ['fails', 'missing']) {
      final shown = await showInFolderFreedesktop(
        '/home/ana/Downloads/plan.txt',
        run: (command, args) async {
          if (command == 'xdg-open' && xdgOpen == 'missing') {
            throw const ProcessException('xdg-open', [], 'not found', 2);
          }
          return ProcessResult(0, 1, '', '');
        },
      );
      expect(shown, isFalse, reason: 'xdg-open $xdgOpen');
    }
  });
}
