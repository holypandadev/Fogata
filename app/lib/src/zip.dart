import 'dart:io';
import 'dart:typed_data';

/// A zip archive made in memory: each file deflated, with plain ASCII names and the
/// time it was made. Enough for a report; nothing here reads archives.
class ZipWriter {
  final _entries = <_Entry>[];

  /// Adds a file named [name] (ASCII, `/` between folders) holding [data].
  void add(String name, List<int> data, {DateTime? time}) {
    final bytes = Uint8List.fromList(data);
    final packed = ZLibCodec(raw: true, level: 9).encode(bytes);
    // Stored as it is when deflating does not make it smaller.
    final deflated = packed.length < bytes.length;
    _entries.add(
      _Entry(
        name: name.codeUnits,
        crc: crc32(bytes),
        size: bytes.length,
        data: deflated ? Uint8List.fromList(packed) : bytes,
        deflated: deflated,
        time: time ?? DateTime.now(),
      ),
    );
  }

  /// The archive's bytes.
  Uint8List bytes() {
    final out = BytesBuilder(copy: false);
    final central = BytesBuilder(copy: false);
    for (final e in _entries) {
      final offset = out.length;
      final (time, date) = _dos(e.time);
      final local = ByteData(30)
        ..setUint32(0, 0x04034b50, Endian.little)
        ..setUint16(4, 20, Endian.little)
        ..setUint16(6, 0, Endian.little)
        ..setUint16(8, e.deflated ? 8 : 0, Endian.little)
        ..setUint16(10, time, Endian.little)
        ..setUint16(12, date, Endian.little)
        ..setUint32(14, e.crc, Endian.little)
        ..setUint32(18, e.data.length, Endian.little)
        ..setUint32(22, e.size, Endian.little)
        ..setUint16(26, e.name.length, Endian.little)
        ..setUint16(28, 0, Endian.little);
      out
        ..add(local.buffer.asUint8List())
        ..add(e.name)
        ..add(e.data);
      final entry = ByteData(46)
        ..setUint32(0, 0x02014b50, Endian.little)
        ..setUint16(4, 20, Endian.little)
        ..setUint16(6, 20, Endian.little)
        ..setUint16(8, 0, Endian.little)
        ..setUint16(10, e.deflated ? 8 : 0, Endian.little)
        ..setUint16(12, time, Endian.little)
        ..setUint16(14, date, Endian.little)
        ..setUint32(16, e.crc, Endian.little)
        ..setUint32(20, e.data.length, Endian.little)
        ..setUint32(24, e.size, Endian.little)
        ..setUint16(28, e.name.length, Endian.little)
        ..setUint32(42, offset, Endian.little);
      central
        ..add(entry.buffer.asUint8List())
        ..add(e.name);
    }
    final start = out.length;
    final directory = central.takeBytes();
    out.add(directory);
    final end = ByteData(22)
      ..setUint32(0, 0x06054b50, Endian.little)
      ..setUint16(8, _entries.length, Endian.little)
      ..setUint16(10, _entries.length, Endian.little)
      ..setUint32(12, directory.length, Endian.little)
      ..setUint32(16, start, Endian.little);
    out.add(end.buffer.asUint8List());
    return out.takeBytes();
  }

  /// MS-DOS time and date, as zip keeps them (two-second steps, from 1980).
  static (int, int) _dos(DateTime t) {
    final year = t.year.clamp(1980, 2107);
    return (
      (t.hour << 11) | (t.minute << 5) | (t.second ~/ 2),
      ((year - 1980) << 9) | (t.month << 5) | t.day,
    );
  }
}

class _Entry {
  _Entry({
    required this.name,
    required this.crc,
    required this.size,
    required this.data,
    required this.deflated,
    required this.time,
  });
  final List<int> name;
  final int crc;
  final int size;
  final Uint8List data;
  final bool deflated;
  final DateTime time;
}

final _crcTable = List<int>.generate(256, (n) {
  var c = n;
  for (var k = 0; k < 8; k++) {
    c = (c & 1) != 0 ? 0xEDB88320 ^ (c >> 1) : c >> 1;
  }
  return c;
});

/// The CRC-32 zip keeps of each file.
int crc32(List<int> data) {
  var c = 0xFFFFFFFF;
  for (final b in data) {
    c = _crcTable[(c ^ b) & 0xFF] ^ (c >> 8);
  }
  return c ^ 0xFFFFFFFF;
}
