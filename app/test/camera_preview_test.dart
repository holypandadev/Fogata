import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/camera_preview.dart';

/// A camera that is never real: it says when it was opened and stopped.
class FakeFeed implements CameraFeed {
  FakeFeed(this.id);
  final String? id;
  bool stopped = false;

  /// Whether it was last drawn mirrored.
  bool? mirrored;

  @override
  Widget view({required bool mirror}) {
    mirrored = mirror;
    return SizedBox(
      key: const ValueKey('fake-camera'),
      child: SizedBox(key: ValueKey(mirror ? 'mirrored' : 'as it is')),
    );
  }

  @override
  Future<void> stop() async => stopped = true;
}

void main() {
  test('nothing opens until start, and stop closes what opened', () async {
    final feeds = <FakeFeed>[];
    final preview = CameraPreview(
      open: (id) async {
        final feed = FakeFeed(id);
        feeds.add(feed);
        return feed;
      },
    );
    expect(preview.on, isFalse);
    expect(feeds, isEmpty);
    await preview.start('cam-1');
    expect(preview.on, isTrue);
    expect(feeds.single.id, 'cam-1');
    expect(preview.feed, same(feeds.single));
    // Started twice, still one camera.
    await preview.start('cam-1');
    expect(feeds, hasLength(1));
    await preview.stop();
    expect(preview.on, isFalse);
    expect(feeds.single.stopped, isTrue);
    // Stopped twice is fine.
    await preview.stop();
  });

  test('a stop while the camera opens closes it once it is open', () async {
    final gate = Completer<void>();
    final feeds = <FakeFeed>[];
    final preview = CameraPreview(
      open: (id) async {
        await gate.future;
        final feed = FakeFeed(id);
        feeds.add(feed);
        return feed;
      },
    );
    unawaited(preview.start(null));
    expect(preview.opening, isTrue);
    var stopped = false;
    final stopping = preview.stop().then((_) => stopped = true);
    await Future<void>.delayed(Duration.zero);
    // The stop waits for the opening, so whoever wants the camera next finds it free.
    expect(stopped, isFalse);
    gate.complete();
    await stopping;
    expect(feeds.single.stopped, isTrue);
    expect(preview.on, isFalse);
    expect(preview.feed, isNull);
  });

  test('a camera that cannot open says so and can be tried again', () async {
    var fail = true;
    final preview = CameraPreview(
      open: (id) async {
        if (fail) throw StateError('no camera');
        return FakeFeed(id);
      },
    );
    var changes = 0;
    preview.addListener(() => changes++);
    await preview.start(null);
    expect(preview.failed, isTrue);
    expect(preview.on, isFalse);
    expect(changes, 2);
    fail = false;
    await preview.start(null);
    expect(preview.failed, isFalse);
    expect(preview.on, isTrue);
    await preview.stop();
  });
}
