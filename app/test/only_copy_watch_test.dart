import 'dart:async';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/window.dart';

import 'wait.dart';

// The copy that runs shows its window when another is started. These use the system's
// real file watching: it is what fails when the folder goes or the limit is reached.
void main() {
  late Directory root;
  late Directory folder;
  setUp(() {
    root = Directory.systemTemp.createTempSync('fogata_show');
    folder = Directory('${root.path}/window')..createSync();
  });
  // Windows lets go of a watched folder a moment after its watch is cancelled: until
  // then it cannot be deleted.
  tearDown(() async {
    for (var tries = 1; ; tries++) {
      try {
        root.deleteSync(recursive: true);
        return;
      } on FileSystemException {
        if (tries == 50) rethrow;
        await Future<void>.delayed(const Duration(milliseconds: 100));
      }
    }
  });

  /// Writes a new request every so often until [done] holds: the system's watch begins
  /// some time after it is asked for (later on a busy computer), and a request written
  /// before then is not seen by it. A request written once the watch is up must be.
  Future<void> askUntil(bool Function() done, {required String what}) async {
    final waited = Stopwatch()..start();
    var n = 0;
    while (!done()) {
      if (waited.elapsed > realWorkLimit) {
        fail('waited ${realWorkLimit.inSeconds} s for $what');
      }
      File('${folder.path}/show').writeAsStringSync('${++n}');
      await until(
        () => done() || waited.elapsed.inMilliseconds > 200 * n,
        what: what,
      );
    }
  }

  test('a request is shown, and the watch goes on after it', () async {
    var shown = 0;
    final stop = watchForShow(folder, () => shown++, (_) {});
    addTearDown(stop);
    await askUntil(() => shown >= 1, what: 'the first request shown');
    final first = shown;
    await askUntil(() => shown > first, what: 'a later request shown');
  });

  test(
    'the error is noted and the watch is made again when the folder is gone',
    () async {
      var shown = 0;
      final errors = <Object>[];
      // The first watch fails when the test says (as the system's does when the folder
      // goes, without waiting on it); the next ones are the system's.
      final first = StreamController<FileSystemEvent>();
      var made = 0;
      final stop = watchForShow(
        folder,
        () => shown++,
        errors.add,
        retry: const Duration(milliseconds: 50),
        longest: const Duration(milliseconds: 200),
        watch: (folder) => made++ == 0
            ? first.stream
            : folder.watch(
                events: FileSystemEvent.create | FileSystemEvent.modify,
              ),
      );
      addTearDown(stop);
      first.addError(const FileSystemException('the folder is gone'));
      await until(() => errors.isNotEmpty, what: 'the error noted');
      await until(() => made >= 2, what: 'the watch made again');
      await askUntil(() => shown >= 1, what: 'a request shown');
    },
  );

  test('a request written while the watch was down is answered', () async {
    var shown = 0;
    final errors = <Object>[];
    // Down (each watch made fails at once, as on a folder that is gone) until the request
    // is written; then made again, with no event of its own: only the request found when
    // it is made again can show the window.
    var down = true;
    var settled = false;
    final stop = watchForShow(
      folder,
      () => shown++,
      errors.add,
      retry: const Duration(milliseconds: 100),
      longest: const Duration(milliseconds: 200),
      watch: (_) => down
          ? Stream.error(const FileSystemException('the folder is gone'))
          : StreamController<FileSystemEvent>().stream,
      known: () => settled = true,
    );
    addTearDown(stop);
    await until(() => settled, what: 'no request known at first');
    await until(() => errors.isNotEmpty, what: 'the watch failing');
    File('${folder.path}/show').writeAsStringSync('1');
    down = false;
    await until(() => shown >= 1, what: 'the request found');
  });

  test(
    'a request is answered whatever the file system makes of its time',
    () async {
      // A file system that keeps whole seconds (FAT, HFS+) gives a request written just
      // after the watch began a time before it.
      var shown = 0;
      final errors = <Object>[];
      // Each watch fails when the test says, as the system's does when the folder goes,
      // and sees no request of its own: only the request found when the watch is made
      // again can show the window.
      final watches = <StreamController<FileSystemEvent>>[];
      var settled = false;
      final stop = watchForShow(
        folder,
        () => shown++,
        errors.add,
        retry: const Duration(milliseconds: 30),
        longest: const Duration(milliseconds: 30),
        watch: (_) {
          final watch = StreamController<FileSystemEvent>();
          watches.add(watch);
          return watch.stream;
        },
        known: () => settled = true,
      );
      addTearDown(stop);
      await until(() => settled, what: 'no request known at first');
      final request = File('${folder.path}/show')..writeAsStringSync('1');
      request.setLastModifiedSync(
        DateTime.now().subtract(const Duration(seconds: 10)),
      );
      watches.last.addError(const FileSystemException('the folder is gone'));
      await until(() => shown >= 1, what: 'the request found');
      // And once only: the request answered is not answered again when the watch is made
      // again once more.
      await until(() => watches.length >= 2, what: 'the watch made again');
      watches.last.addError(const FileSystemException('the folder is gone'));
      await until(() => watches.length >= 3, what: 'the watch made again');
      await Future<void>.delayed(const Duration(milliseconds: 300));
      expect(shown, 1);
    },
  );

  test('ending it stops the retries', () async {
    // A watch that fails as soon as it is made, as the system's does on a folder that is
    // gone, without waiting on the system (slow to say so on a busy computer).
    final errors = <Object>[];
    var made = 0;
    final stop = watchForShow(
      folder,
      () {},
      errors.add,
      retry: const Duration(milliseconds: 30),
      longest: const Duration(milliseconds: 30),
      watch: (_) {
        made++;
        return Stream.error(const FileSystemException('the folder is gone'));
      },
    );
    await until(() => errors.length >= 2, what: 'two failures');
    stop();
    final (count, tries) = (errors.length, made);
    await Future<void>.delayed(const Duration(milliseconds: 300));
    expect(errors.length, count);
    expect(made, tries, reason: 'made again after it ended');
  });
}
