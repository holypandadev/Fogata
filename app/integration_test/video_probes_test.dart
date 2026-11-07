// Reading videos' first pictures with this system's own player: several at once, and
// some still under way when the application closes. A file that stalls is checked in the
// plugin's own tests (windows/test), since probeVideo takes local videos only. The clip
// (integration_test/clip.mp4) is found next to the working folder or given in FOGATA_CLIP.
import 'dart:async';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:fogata_video/fogata_video.dart';
import 'package:integration_test/integration_test.dart';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  final clip =
      Platform.environment['FOGATA_CLIP'] ??
      '${Directory.current.path}/integration_test/clip.mp4';

  testWidgets('six videos at once: two are read, the others answered at once', (
    tester,
  ) async {
    final started = Stopwatch()..start();
    final probes = await Future.wait([
      for (var i = 0; i < 6; i++) probeVideo(clip, maxWidth: 160),
    ]);
    final read = probes.whereType<VideoProbe>().length;
    // ignore: avoid_print
    print('six at once: $read read in ${started.elapsedMilliseconds} ms');
    expect(read, inInclusiveRange(1, 2));
    // Once those are done, a video is read again.
    expect(await probeVideo(clip, maxWidth: 160), isNotNull);
  });

  testWidgets('the application closes with readings still under way', (
    tester,
  ) async {
    // Not waited for: the test ends, and the application with it, while the system may
    // still be reading them. The plugin then has nobody to answer, and must not fail.
    for (var i = 0; i < 2; i++) {
      unawaited(probeVideo(clip, maxWidth: 160));
    }
  });
}
