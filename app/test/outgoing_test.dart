import 'dart:async';
import 'dart:io';
import 'dart:typed_data';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/outgoing.dart';
import 'package:fogata/src/safe_image.dart';
import 'package:fogata/src/theme.dart';

import 'wait.dart';

/// CRC-32 as PNG chunks use it.
int _crc(List<int> bytes) {
  var crc = 0xffffffff;
  for (final b in bytes) {
    crc ^= b;
    for (var i = 0; i < 8; i++) {
      crc = (crc & 1) != 0 ? (crc >> 1) ^ 0xedb88320 : crc >> 1;
    }
  }
  return crc ^ 0xffffffff;
}

List<int> _chunk(String type, List<int> data) {
  final body = [...type.codeUnits, ...data];
  final length = ByteData(4)..setUint32(0, data.length);
  final crc = ByteData(4)..setUint32(0, _crc(body));
  return [...length.buffer.asUint8List(), ...body, ...crc.buffer.asUint8List()];
}

/// A PNG whose header says 30000 by 30000 pixels: a few kilobytes that would ask for
/// gigabytes once decoded.
Uint8List _bomb() {
  final header = ByteData(13)
    ..setUint32(0, 30000)
    ..setUint32(4, 30000)
    ..setUint8(8, 8)
    ..setUint8(9, 6);
  return Uint8List.fromList([
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, //
    ..._chunk('IHDR', header.buffer.asUint8List()),
    ..._chunk('IDAT', ZLibCodec().encode(Uint8List(64 * 1024))),
    ..._chunk('IEND', const []),
  ]);
}

Future<Uint8List> _small() async {
  final recorder = ui.PictureRecorder();
  ui.Canvas(recorder).drawRect(
    const ui.Rect.fromLTWH(0, 0, 64, 64),
    ui.Paint()..color = const ui.Color(0xff5a1e9e),
  );
  final image = await recorder.endRecording().toImage(64, 64);
  final data = await image.toByteData(format: ui.ImageByteFormat.png);
  return data!.buffer.asUint8List();
}

void main() {
  late Directory dir;
  late String plan;
  late String photos;

  setUp(() {
    dir = Directory.systemTemp.createTempSync('fogata-outgoing-test');
    plan = '${dir.path}/plan.pdf';
    photos = '${dir.path}/photos.zip';
    File(plan).writeAsBytesSync(List.filled(2048, 1));
    File(photos).writeAsBytesSync(List.filled(4096, 2));
    Directory('${dir.path}/folder').createSync();
  });

  tearDown(() => dir.deleteSync(recursive: true));

  test(
    'files wait for Send; folders and the same file twice are left out',
    () async {
      final state = AppState('/nonexistent/fogata-outgoing-test');
      state.roomState.roomGroup = 'group';
      state.roomState.roomTab = 1;
      await state.addOutgoing([
        plan,
        '${dir.path}/folder',
        photos,
        plan,
        '${dir.path}/gone.txt',
      ]);
      expect(state.roomState.outgoingHere.map((f) => f.name), [
        'plan.pdf',
        'photos.zip',
      ]);
      expect(state.roomState.outgoingHere.map((f) => f.size), [2048, 4096]);
      expect(
        state.toasts.list.single.text,
        contains('the folder was left out'),
      );
      // The chat shows them, with the message box.
      expect(state.roomState.roomTab, 0);

      state.removeOutgoing(state.roomState.outgoingHere.first);
      expect(state.roomState.outgoingHere.map((f) => f.name), ['photos.zip']);

      // Another room's files are its own.
      state.roomState.roomGroup = 'other';
      expect(state.roomState.outgoingHere, isEmpty);
      state.roomState.roomGroup = 'group';
      state.clearOutgoing();
      expect(state.roomState.outgoingHere, isEmpty);
      expect(state.roomState.outgoing, isEmpty);
    },
  );

  test(
    'only files of this computer wait: no text, device or pipe read as one',
    () async {
      final state = AppState('/nonexistent/fogata-outgoing-test');
      state.roomState.roomGroup = 'group';
      final link = '${dir.path}/link.pdf';
      Link(link).createSync(plan);
      final pipe = '${dir.path}/pipe';
      final piped = Platform.isWindows
          ? false
          : (await Process.run('mkfifo', [pipe])).exitCode == 0;
      await state.addOutgoing([
        // Text another application dropped, read from where the application started.
        'pubspec.yaml',
        if (!Platform.isWindows) '/dev/urandom',
        if (piped) pipe,
        link,
      ]);
      expect(state.roomState.outgoingHere.map((f) => f.name), ['link.pdf']);
      // Where the bytes come from, links followed.
      expect(
        state.roomState.outgoingHere.single.folder,
        Directory(dir.path).resolveSymbolicLinksSync(),
      );
    },
  );

  test('a folder in the home shows from ~', () {
    final sep = Platform.pathSeparator;
    final home = Platform.isWindows ? r'C:\Users\leo' : '/home/leo';
    final env = {'HOME': home, 'USERPROFILE': home};
    expect(shownFolder('$home$sep.ssh', environment: env), '~$sep.ssh');
    expect(shownFolder(home, environment: env), '~');
    expect(
      shownFolder('${home}na${sep}x', environment: env),
      '${home}na${sep}x',
    );
    // A long one ends where it is told apart: a line cut at its end would hide that.
    expect(
      shownFolder(
        '$home${sep}Projects${sep}tests${sep}run${sep}home$sep.ssh',
        environment: env,
      ),
      '…${sep}home$sep.ssh',
    );
  });

  testWidgets('the tray lists the files with their sizes and removes them', (
    tester,
  ) async {
    final state = AppState('/nonexistent/fogata-outgoing-test');
    state.roomState.roomGroup = 'group';
    // Real file reads: they finish outside the test's own time.
    await tester.runAsync(() => state.addOutgoing([plan, photos]));
    await tester.pumpWidget(
      AppScope(
        state: state,
        child: MaterialApp(
          theme: Fg.darkTokens.themeData,
          home: Scaffold(
            body: ListenableBuilder(
              listenable: state.roomState,
              builder: (context, _) => OutgoingTray(
                state: state,
                files: state.roomState.outgoingHere,
              ),
            ),
          ),
        ),
      ),
    );
    expect(find.text('plan.pdf'), findsOneWidget);
    expect(find.text('photos.zip'), findsOneWidget);
    expect(find.textContaining('files to share'), findsOneWidget);
    // Each with the folder it comes from.
    final folder = shownFolder(Directory(dir.path).resolveSymbolicLinksSync());
    expect(find.textContaining(folder, findRichText: true), findsNWidgets(2));

    await tester.tap(find.byTooltip('Remove plan.pdf'));
    await tester.pump();
    expect(find.text('plan.pdf'), findsNothing);
    expect(find.textContaining('file to share'), findsOneWidget);

    await tester.tap(find.text('Remove all'));
    await tester.pump();
    expect(state.roomState.outgoingHere, isEmpty);
  });

  testWidgets('a picture waiting to go is decoded only if its header fits', (
    tester,
  ) async {
    final huge = '${dir.path}/huge.png';
    final small = '${dir.path}/small.png';
    final state = AppState('/nonexistent/fogata-outgoing-test');
    state.roomState.roomGroup = 'group';
    await tester.runAsync(() async {
      File(huge).writeAsBytesSync(_bomb());
      File(small).writeAsBytesSync(await _small());
      await state.addOutgoing([huge, small]);
    });
    await tester.pumpWidget(
      AppScope(
        state: state,
        child: MaterialApp(
          theme: Fg.darkTokens.themeData,
          home: Scaffold(
            body: OutgoingTray(
              state: state,
              files: state.roomState.outgoingHere,
            ),
          ),
        ),
      ),
    );
    // The files are read, and their headers checked, outside the test's own time: until
    // both rows show a picture rather than a file's icon.
    await pumpUntil(
      tester,
      () => find.byType(Image).evaluate().length == 2,
      what: 'both pictures read from their files',
    );
    final images = tester.widgetList<Image>(find.byType(Image)).toList();
    // No picture straight from its file, whatever its header says.
    expect(images.where((i) => i.image is FileImage), isEmpty);
    expect(images, hasLength(2));
    for (final image in images) {
      final provider = image.image;
      expect(provider, isA<ResizeImage>());
      expect((provider as ResizeImage).imageProvider, isA<SafeMemoryImage>());
    }
    // The small one shows; the one too large to decode shows as a file.
    final shown = [
      for (final image in images)
        await tester.runAsync(() async {
          final done = Completer<bool>();
          image.image
              .resolve(ImageConfiguration.empty)
              .addListener(
                ImageStreamListener(
                  (info, _) {
                    info.dispose();
                    if (!done.isCompleted) done.complete(true);
                  },
                  onError: (_, _) {
                    if (!done.isCompleted) done.complete(false);
                  },
                ),
              );
          return done.future;
        }),
    ];
    expect(shown, [false, true]);
  });
}
