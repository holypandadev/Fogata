import 'dart:io';
import 'dart:typed_data';

import 'package:file_selector/file_selector.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/photo.dart';

void main() {
  late Directory dir;
  setUp(() => dir = Directory.systemTemp.createTempSync('fogata_picked'));
  tearDown(() => dir.deleteSync(recursive: true));

  test('a file within the limit is read whole', () async {
    final f = File('${dir.path}/a.png')..writeAsBytesSync(Uint8List(1000));
    expect((await readPicked(XFile(f.path), maxBytes: 1000)).length, 1000);
  });

  test('a file over the limit on disk is refused without reading it', () async {
    final f = File('${dir.path}/big.png')..writeAsBytesSync(Uint8List(1001));
    await expectLater(
      readPicked(XFile(f.path), maxBytes: 1000),
      throwsA(isA<PictureProblem>()),
    );
  });

  test('a file that grows after its size was looked at is caught', () async {
    final f = File('${dir.path}/grows.png')..writeAsBytesSync(Uint8List(900));
    final file = _Growing(
      f.path,
      onLength: () {
        f.writeAsBytesSync(Uint8List(5000), mode: FileMode.append);
      },
    );
    await expectLater(
      readPicked(file, maxBytes: 1000),
      throwsA(isA<PictureProblem>()),
    );
  });

  test(
    'reading stops at the limit instead of taking what never ends',
    () async {
      var chunks = 0;
      final endless = () async* {
        while (true) {
          chunks++;
          yield Uint8List(100);
        }
      }();
      expect(await readAtMost(endless, 1000), isNull);
      expect(chunks, lessThan(20));
    },
  );
}

/// A picked file whose size is read before it grows.
class _Growing extends XFile {
  _Growing(super.path, {required this.onLength});
  final void Function() onLength;

  @override
  Future<int> length() async {
    final n = await super.length();
    onLength();
    return n;
  }
}
