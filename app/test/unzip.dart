// Reads the archives a problem report is, for the tests.

import 'dart:io';
import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/zip.dart';

/// The files of a zip archive, read from its central directory.
Map<String, List<int>> unzip(Uint8List zip) {
  final data = ByteData.sublistView(zip);
  var end = zip.length - 22;
  while (data.getUint32(end, Endian.little) != 0x06054b50) {
    end--;
  }
  final count = data.getUint16(end + 10, Endian.little);
  var at = data.getUint32(end + 16, Endian.little);
  final out = <String, List<int>>{};
  for (var i = 0; i < count; i++) {
    expect(data.getUint32(at, Endian.little), 0x02014b50);
    final method = data.getUint16(at + 10, Endian.little);
    final crc = data.getUint32(at + 16, Endian.little);
    final packed = data.getUint32(at + 20, Endian.little);
    final size = data.getUint32(at + 24, Endian.little);
    final nameLength = data.getUint16(at + 28, Endian.little);
    final local = data.getUint32(at + 42, Endian.little);
    final name = String.fromCharCodes(
      zip.sublist(at + 46, at + 46 + nameLength),
    );
    expect(data.getUint32(local, Endian.little), 0x04034b50);
    final start =
        local +
        30 +
        data.getUint16(local + 26, Endian.little) +
        data.getUint16(local + 28, Endian.little);
    final stored = zip.sublist(start, start + packed);
    final bytes = method == 8
        ? ZLibCodec(raw: true).decode(stored)
        : stored.toList();
    expect(bytes.length, size, reason: name);
    expect(crc32(bytes), crc, reason: name);
    out[name] = bytes;
    at += 46 + nameLength;
  }
  return out;
}
