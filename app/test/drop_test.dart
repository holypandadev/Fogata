import 'dart:async';
import 'dart:io';

import 'package:desktop_drop/desktop_drop.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  testWidgets('a drop on Linux gives only the local files of its list', (
    tester,
  ) async {
    final dropped = <String>[];
    await tester.pumpWidget(
      DropTarget(
        onDragDone: (details) =>
            dropped.addAll(details.files.map((f) => f.path)),
        child: const SizedBox.expand(),
      ),
    );
    // What the Linux part of the plugin hands over: the text of a drop, where it was
    // dropped, and the target its data came as, which is always a text/uri-list.
    final list = [
      '# a comment',
      '.ssh/id_ed25519',
      '/dev/urandom',
      'https://example.com/sticker.png',
      'smb://server/share/plan.pdf',
      'file://otherhost/home/leo/plan.pdf',
      'file:///home/leo/plan.pdf?x=1',
      'file:///home/leo/Pictures/blue%20lake.jpg',
      'file://localhost/home/leo/notes.txt',
      '',
    ].join('\r\n');
    final done = Completer<ByteData?>();
    await tester.binding.defaultBinaryMessenger.handlePlatformMessage(
      'desktop_drop',
      const StandardMethodCodec().encodeMethodCall(
        MethodCall('performOperation_linux', [
          list,
          [10.0, 20.0],
          'text/uri-list',
        ]),
      ),
      done.complete,
    );
    await done.future;
    expect(dropped, [
      '/home/leo/Pictures/blue lake.jpg',
      '/home/leo/notes.txt',
    ]);
    // The drop target takes a drop this way only on Linux.
  }, skip: !Platform.isLinux);
}
