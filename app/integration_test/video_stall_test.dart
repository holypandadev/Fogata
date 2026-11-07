// Linux: videos that stop coming, as a file on a network share that hangs does. Each is a
// named pipe that gives the head of a video and then nothing, while it stays open. The
// player opens only plain files, which it checks and then reads from the file it opened:
// a pipe is refused at once, without waiting for it, and gives its turn back. The clip
// (integration_test/clip.mp4) is found next to the working folder or given in FOGATA_CLIP.
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:fogata_video/fogata_video.dart';
import 'package:integration_test/integration_test.dart';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  final clip =
      Platform.environment['FOGATA_CLIP'] ??
      '${Directory.current.path}/integration_test/clip.mp4';

  /// A pipe named [name] in [dir] that gives the clip's first 12 bytes, then nothing until
  /// its writer is killed.
  Future<String> stalling(
    Directory dir,
    String name,
    List<Process> writers,
  ) async {
    final path = '${dir.path}/$name';
    final made = await Process.run('mkfifo', [path]);
    expect(made.exitCode, 0, reason: '${made.stderr}');
    writers.add(
      await Process.start('sh', [
        '-c',
        r'exec 3>"$1"; head -c 12 "$2" >&3; exec sleep 600',
        'sh',
        path,
        clip,
      ]),
    );
    return path;
  }

  testWidgets(
    'videos that stop coming are answered in their time, and others still read',
    (tester) async {
      final dir = await Directory.systemTemp.createTemp('fogata-stall');
      final writers = <Process>[];
      addTearDown(() async {
        for (final w in writers) {
          w.kill();
        }
        await dir.delete(recursive: true);
      });
      // Both of the plugin's readings at once, on files that never come: not read.
      final started = Stopwatch()..start();
      final stalled = await Future.wait([
        for (var i = 0; i < 2; i++)
          probeVideo(
            await stalling(dir, 'stall-$i.mp4', writers),
            maxWidth: 160,
          ).timeout(const Duration(seconds: 30)),
      ]);
      // ignore: avoid_print
      print(
        'two stalled readings answered in ${started.elapsedMilliseconds} ms',
      );
      expect(stalled, [null, null]);
      expect(started.elapsed, lessThan(const Duration(seconds: 20)));
      // Their turns came back, though their files still hang.
      expect(
        await probeVideo(
          clip,
          maxWidth: 160,
        ).timeout(const Duration(seconds: 30)),
        isNotNull,
      );

      // Nor does a player of one open, and it is not waited for either.
      final opening = Stopwatch()..start();
      final player = await VideoPlayer.open(
        await stalling(dir, 'stall-play.mp4', writers),
      ).timeout(const Duration(seconds: 10));
      expect(player, isNull);
      expect(opening.elapsed, lessThan(const Duration(seconds: 2)));

      // Once the files come to an end, what they held is let go of, and videos are read.
      for (final w in writers) {
        w.kill();
      }
      await Future<void>.delayed(const Duration(seconds: 2));
      for (var i = 0; i < 2; i++) {
        expect(
          await probeVideo(
            clip,
            maxWidth: 160,
          ).timeout(const Duration(seconds: 30)),
          isNotNull,
        );
      }
    },
    skip: !Platform.isLinux,
  );
}
