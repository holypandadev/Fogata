import 'dart:async';
import 'dart:io';
import 'dart:typed_data';

import 'package:fake_async/fake_async.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/outgoing_send.dart';
import 'package:fogata_video/fogata_video.dart';

/// Runs [body] in fake time, the clock of the video turns with it: a reading's 20 ms and
/// a wait's 200 ms are then what they say, however busy the computer is.
void inFakeTime(Future<void> Function() body) {
  fakeAsync((time) {
    videoProbeClock = () => time.elapsed;
    var done = false;
    Object? error;
    StackTrace? trace;
    body().then(
      (_) => done = true,
      onError: (Object e, StackTrace t) {
        (error, trace, done) = (e, t, true);
      },
    );
    time.elapse(const Duration(minutes: 1));
    if (error != null) Error.throwWithStackTrace(error!, trace!);
    expect(done, isTrue, reason: 'it ended within a minute of fake time');
    // Every reading ended, so every turn is back: none is left to the next test.
    expect(videoTurnsHeld, 0, reason: 'turns still held');
  });
}

final realVideoProbeClock = videoProbeClock;

void main() {
  late Directory dir;
  late AppState state;
  setUp(() {
    dir = Directory.systemTemp.createTempSync('fogata_send');
    state = AppState('/nonexistent/fogata-outgoing-send-test');
    state.roomState.roomGroup = 'group';
  });
  tearDown(() {
    videoProbe = probeVideo;
    videoProbeTime = const Duration(seconds: 15);
    videoProbeHeldMost = const Duration(minutes: 1);
    videoProbeClock = realVideoProbeClock;
    dir.deleteSync(recursive: true);
  });

  List<String> make(int n, {String ext = 'pdf'}) {
    final paths = [
      for (var i = 0; i < n; i++)
        (File(
          '${dir.path}/f${i.toString().padLeft(3, '0')}.$ext',
        )..writeAsStringSync('x$i')).path,
    ];
    return paths;
  }

  List<String> waiting() => [
    for (final f in state.roomState.outgoingHere) f.path.split('/').last,
  ];

  group('room', () {
    test('a failure on the first, a middle and the last file waits again, '
        'and a retry sends only those', () async {
      final paths = make(5);
      await state.addOutgoing(paths);
      final failing = {paths[0], paths[2], paths[4]};
      final shared = <String>[];
      state.shareFile = ({required groupId, required path, video}) async {
        if (failing.contains(path)) throw Exception('disk said no');
        shared.add(path);
        return 'id';
      };
      await state.sendOutgoing('');
      expect(shared, [paths[1], paths[3]]);
      expect(waiting(), ['f000.pdf', 'f002.pdf', 'f004.pdf']);
      expect(
        state.roomState.outgoingHere.map((f) => f.state),
        everyElement(OutgoingState.failed),
      );
      // Retried without picking them again.
      failing.clear();
      await state.sendOutgoing('');
      expect(shared, [paths[1], paths[3], paths[0], paths[2], paths[4]]);
      expect(waiting(), isEmpty);
    });

    test('files added while sending stay, and none is added twice', () async {
      final paths = make(3);
      await state.addOutgoing(paths);
      final gate = Completer<void>();
      final going = Completer<void>();
      state.shareFile = ({required groupId, required path, video}) async {
        if (!going.isCompleted) going.complete();
        await gate.future;
        if (path == paths[1]) throw Exception('no');
        return 'id';
      };
      final sending = state.sendOutgoing('');
      await going.future;
      // Picked again while it goes, with a new one.
      final more = make(1, ext: 'txt');
      await state.addOutgoing([paths[1], ...more]);
      gate.complete();
      await sending;
      expect(waiting(), ['f001.pdf', 'f000.txt']);
    });

    test('a file being sent is not added again, and a failed one comes back '
        'without changing the tab', () async {
      final paths = make(3);
      await state.addOutgoing(paths);
      final gate = Completer<void>();
      final going = Completer<void>();
      state.shareFile = ({required groupId, required path, video}) async {
        if (!going.isCompleted) going.complete();
        await gate.future;
        if (path == paths[2]) throw Exception('no');
        return 'id';
      };
      final sending = state.sendOutgoing('');
      await going.future;
      // Picked again while it goes: one that will be shared, and one that will fail.
      await state.addOutgoing([paths[0], paths[2]]);
      expect(waiting(), isEmpty);
      // The person goes to another tab while it is sent.
      state.roomState.roomTab = 2;
      gate.complete();
      await sending;
      expect(waiting(), ['f002.pdf']);
      expect(state.roomState.roomTab, 2);
      // Once sent, a file can be picked again.
      await state.addOutgoing([paths[0]]);
      expect(waiting(), ['f002.pdf', 'f000.pdf']);
    });

    test('files are not sent on after the room was left', () async {
      await state.addOutgoing(make(4));
      final tried = <String>[];
      state.shareFile = ({required groupId, required path, video}) async {
        tried.add(path);
        state.roomState.roomGroup = 'other';
        return 'id';
      };
      await state.sendOutgoing('');
      expect(tried, hasLength(1));
    });

    test('the room taking no more files keeps the files not tried', () async {
      final paths = make(4);
      await state.addOutgoing(paths);
      var calls = 0;
      state.shareFile = ({required groupId, required path, video}) async {
        calls++;
        if (calls == 2) throw 'files in this room already';
        return 'id';
      };
      await state.sendOutgoing('');
      expect(calls, 2);
      expect(waiting(), ['f001.pdf', 'f002.pdf', 'f003.pdf']);
    });

    test('a video whose picture is read while the room is left, or left and '
        'entered again, is not shared there', () async {
      VideoProbe probe() =>
          VideoProbe(thumb: Uint8List(1), width: 2, height: 2, durationMs: 1);
      final tried = <String>[];
      state.shareFile = ({required groupId, required path, video}) async {
        tried.add(path);
        return 'id';
      };
      // Left.
      await state.addOutgoing(make(1, ext: 'mp4'));
      videoProbe = (path) async {
        state.roomState.roomGroup = 'other';
        return probe();
      };
      await state.sendOutgoing('');
      expect(tried, isEmpty);
      // Left and entered again: another visit of the same room.
      state.roomState.roomGroup = 'group';
      state.roomState.memories['group'] = RoomMemory(1);
      await state.addOutgoing(make(2, ext: 'mp4'));
      videoProbe = (path) async {
        state.roomState.memories['group'] = RoomMemory(1);
        return probe();
      };
      await state.sendOutgoing('');
      expect(tried, isEmpty);
      expect(waiting(), hasLength(2));
      // Still the same visit: shared.
      videoProbe = (path) async => probe();
      await state.sendOutgoing('');
      expect(tried, hasLength(2));
    });

    test('files do not come back to a room that was left', () async {
      await state.addOutgoing(make(2));
      state.shareFile = ({required groupId, required path, video}) async {
        state.roomState.roomGroup = 'other';
        throw Exception('not in the room');
      };
      await state.sendOutgoing('');
      expect(state.roomState.outgoing['group'], isNull);
    });
  });

  group('direct', () {
    test(
      'at most 10 videos are probed, 2 at a time, and the rest wait',
      () async {
        final paths = make(100, ext: 'mp4');
        state.direct.open = 'friend';
        await state.direct.addOutgoing(paths);
        var running = 0, most = 0;
        final probed = <String>[];
        videoProbe = (path) async {
          probed.add(path);
          running++;
          most = running > most ? running : most;
          await Future<void>.delayed(const Duration(milliseconds: 10));
          running--;
          return VideoProbe(
            thumb: Uint8List(1),
            width: 2,
            height: 2,
            durationMs: 1,
          );
        };
        var sentCount = -1;
        state.direct.sendDirect =
            ({required deviceId, required text, required files}) async {
              sentCount = files.length;
            };
        await state.direct.send("hi");
        expect(probed.length, 10);
        expect(most, lessThanOrEqualTo(2));
        expect(sentCount, 10);
        expect(state.direct.waiting.length, 90);
      },
    );

    test(
      'a state let go of while videos are read sends nothing and says no',
      () async {
        state.direct.open = 'friend';
        await state.direct.addOutgoing(make(4, ext: 'mp4'));
        videoProbe = (path) async {
          state.dispose();
          return null;
        };
        var told = false;
        state.direct.sendDirect =
            ({required deviceId, required text, required files}) async {
              told = true;
            };
        expect(await state.direct.send('hi'), isFalse);
        expect(told, isFalse);
      },
    );

    test('a failed send puts the files back, with the unsent ones', () async {
      final paths = make(12, ext: 'pdf');
      state.direct.open = 'friend';
      await state.direct.addOutgoing(paths);
      state.direct.sendDirect =
          ({required deviceId, required text, required files}) async {
            throw Exception('no');
          };
      expect(await state.direct.send(''), isFalse);
      expect(state.direct.waiting.map((f) => f.path), paths);
    });
  });

  test(
    'videos are read two at a time in the whole application, a turn held until the '
    'system is done',
    () => inFakeTime(() async {
      videoProbeTime = const Duration(milliseconds: 200);
      var running = 0, most = 0;
      final hung = Completer<void>();
      videoProbe = (path) async {
        running++;
        most = running > most ? running : most;
        // The first never ends until let go of; the others take a moment.
        if (path.endsWith('f000.mp4')) {
          await hung.future;
        } else {
          await Future<void>.delayed(const Duration(milliseconds: 20));
        }
        running--;
        return VideoProbe(
          thumb: Uint8List(1),
          width: 2,
          height: 2,
          durationMs: 1,
        );
      };
      final files = [
        for (final path in make(7, ext: 'mp4'))
          OutgoingFile(path, path.split('/').last, 3),
      ];
      // Three sends at once, as from three chats and a room.
      final videos = await Future.wait([
        Future.wait([for (final f in files.take(3)) videoFor(f)]),
        Future.wait([for (final f in files.skip(3).take(2)) videoFor(f)]),
        Future.wait([for (final f in files.skip(5)) videoFor(f)]),
      ]);
      expect(most, lessThanOrEqualTo(2));
      // The one that hung went without its picture, the others with theirs.
      expect(videos[0][0], isNull);
      expect(
        [...videos[0].skip(1), ...videos[1], ...videos[2]].nonNulls,
        hasLength(6),
      );
      // While it still runs it keeps its turn: only one more reads at a time.
      final more = [
        for (final path in make(9, ext: 'mp4').skip(7))
          OutgoingFile(path, path.split('/').last, 3),
      ];
      running = 0;
      most = 0;
      await Future.wait([for (final f in more) videoFor(f)]);
      expect(most, 1);
      hung.complete();
      await pumpEventQueue();
      running = 0;
      most = 0;
      await Future.wait([for (final f in more) videoFor(f)]);
      expect(most, 2);
    }),
  );

  test(
    'readings that never end give their turns back after a while, once',
    () => inFakeTime(() async {
      videoProbeTime = const Duration(milliseconds: 200);
      videoProbeHeldMost = const Duration(milliseconds: 400);
      var running = 0, most = 0;
      final stuck = Completer<void>();
      videoProbe = (path) async {
        running++;
        most = running > most ? running : most;
        if (path.endsWith('f000.mp4') || path.endsWith('f001.mp4')) {
          await stuck.future;
        } else {
          await Future<void>.delayed(const Duration(milliseconds: 20));
        }
        running--;
        return VideoProbe(
          thumb: Uint8List(1),
          width: 2,
          height: 2,
          durationMs: 1,
        );
      };
      final files = [
        for (final path in make(4, ext: 'mp4'))
          OutgoingFile(path, path.split('/').last, 3),
      ];
      // Two readings stuck for good hold both turns: the next waits its time for nothing.
      final first = await Future.wait([
        for (final f in files.take(2)) videoFor(f),
      ]);
      expect(first, [null, null]);
      expect(await videoFor(files[2]), isNull);
      // Past the most they hold one, the turns are back: the next is read.
      await Future<void>.delayed(const Duration(milliseconds: 300));
      expect(await videoFor(files[3]), isNotNull);
      // When the stuck ones end at last, no turn is given back twice: still two at most.
      stuck.complete();
      await pumpEventQueue();
      running = 0;
      most = 0;
      final again = [
        for (final path in make(9, ext: 'mp4').skip(4))
          OutgoingFile(path, path.split('/').last, 3),
      ];
      await Future.wait([for (final f in again) videoFor(f)]);
      expect(most, 2);
    }),
  );

  test('the pool stops starting work when asked to', () async {
    var stopped = false;
    final started = <int>[];
    final r = await inPool<int, int>([1, 2, 3, 4, 5, 6], 2, (i) async {
      started.add(i);
      await Future<void>.delayed(const Duration(milliseconds: 5));
      if (i == 2) stopped = true;
      return i;
    }, stop: () => stopped);
    expect(started.length, lessThan(6));
    expect(r.where((e) => e == null), isNotEmpty);
  });

  test(
    'the pool starts no more work once one has failed, and says so',
    () async {
      final started = <int>[];
      await expectLater(
        inPool<int, int>([1, 2, 3, 4], 2, (i) async {
          started.add(i);
          if (i == 1) throw StateError('unreadable');
          await Future<void>.delayed(const Duration(milliseconds: 20));
          return i;
        }),
        throwsStateError,
      );
      expect(started, [1, 2]);
    },
  );
}
