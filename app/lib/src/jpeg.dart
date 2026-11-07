import 'dart:math' as math;
import 'dart:typed_data';

/// Encodes RGBA pixels (`width` by `height`, alpha ignored) as a baseline JPEG, with the
/// standard tables of the JPEG specification (ITU-T T.81, Annex K) scaled to `quality`
/// (1 to 100) and no colour subsampling. A photo keeps far more detail in a few tens of
/// kilobytes than as a PNG.
Uint8List encodeJpeg(
  Uint8List rgba,
  int width,
  int height, {
  int quality = 85,
}) {
  assert(rgba.length >= width * height * 4);
  final luma = _scaled(_lumaTable, quality);
  final chroma = _scaled(_chromaTable, quality);
  final out = BytesBuilder(copy: false);
  void marker(int code, List<int> body) {
    out.add([0xFF, code, (body.length + 2) >> 8, (body.length + 2) & 0xFF]);
    out.add(body);
  }

  out.add([0xFF, 0xD8]);
  marker(0xE0, [0x4A, 0x46, 0x49, 0x46, 0, 1, 1, 0, 0, 1, 0, 1, 0, 0]);
  marker(0xDB, [
    0,
    for (final i in _zigzag) luma[i],
    1,
    for (final i in _zigzag) chroma[i],
  ]);
  marker(0xC0, [
    8,
    height >> 8,
    height & 0xFF,
    width >> 8,
    width & 0xFF,
    3,
    1, 0x11, 0, // Y
    2, 0x11, 1, // Cb
    3, 0x11, 1, // Cr
  ]);
  marker(0xC4, [
    for (final (id, table) in [
      (0x00, _dcLuma),
      (0x10, _acLuma),
      (0x01, _dcChroma),
      (0x11, _acChroma),
    ]) ...[id, ...table.bits, ...table.values],
  ]);
  marker(0xDA, [3, 1, 0x00, 2, 0x11, 3, 0x11, 0, 63, 0]);

  final bits = _Bits(out);
  final dcLuma = _codes(_dcLuma);
  final acLuma = _codes(_acLuma);
  final dcChroma = _codes(_dcChroma);
  final acChroma = _codes(_acChroma);
  final previous = [0, 0, 0];
  final block = List.generate(3, (_) => Float64List(64));
  for (var by = 0; by < height; by += 8) {
    for (var bx = 0; bx < width; bx += 8) {
      // Edges past the picture repeat its last row and column.
      for (var y = 0; y < 8; y++) {
        final row = (by + y < height ? by + y : height - 1) * width;
        for (var x = 0; x < 8; x++) {
          final p = (row + (bx + x < width ? bx + x : width - 1)) * 4;
          final r = rgba[p].toDouble();
          final g = rgba[p + 1].toDouble();
          final b = rgba[p + 2].toDouble();
          final i = y * 8 + x;
          block[0][i] = .299 * r + .587 * g + .114 * b - 128;
          block[1][i] = -.168736 * r - .331264 * g + .5 * b;
          block[2][i] = .5 * r - .418688 * g - .081312 * b;
        }
      }
      for (var c = 0; c < 3; c++) {
        previous[c] = _block(
          bits,
          block[c],
          c == 0 ? luma : chroma,
          c == 0 ? dcLuma : dcChroma,
          c == 0 ? acLuma : acChroma,
          previous[c],
        );
      }
    }
  }
  bits.flush();
  out.add([0xFF, 0xD9]);
  return out.takeBytes();
}

/// Transforms, quantizes and writes one 8 by 8 block; returns its DC value for the next.
int _block(
  _Bits bits,
  Float64List samples,
  Int32List table,
  List<(int, int)> dc,
  List<(int, int)> ac,
  int previous,
) {
  final rows = Float64List(64);
  for (var y = 0; y < 8; y++) {
    for (var u = 0; u < 8; u++) {
      var sum = 0.0;
      for (var x = 0; x < 8; x++) {
        sum += _cos[u * 8 + x] * samples[y * 8 + x];
      }
      rows[y * 8 + u] = sum;
    }
  }
  final coefficients = Int32List(64);
  for (var u = 0; u < 8; u++) {
    for (var v = 0; v < 8; v++) {
      var sum = 0.0;
      for (var y = 0; y < 8; y++) {
        sum += _cos[v * 8 + y] * rows[y * 8 + u];
      }
      final i = v * 8 + u;
      coefficients[i] = (sum / table[i]).round();
    }
  }
  final value = coefficients[0];
  _put(bits, dc, value - previous, 0);
  var zeros = 0;
  for (var k = 1; k < 64; k++) {
    final coefficient = coefficients[_zigzag[k]];
    if (coefficient == 0) {
      zeros++;
      continue;
    }
    while (zeros > 15) {
      bits.write(ac[0xF0]);
      zeros -= 16;
    }
    _put(bits, ac, coefficient, zeros);
    zeros = 0;
  }
  if (zeros > 0) bits.write(ac[0x00]);
  return value;
}

/// Writes a value as its size's Huffman code (after `zeros` zeros for AC) and its bits.
void _put(_Bits bits, List<(int, int)> codes, int value, int zeros) {
  final magnitude = value.abs();
  var size = 0;
  while ((1 << size) <= magnitude) {
    size++;
  }
  bits.write(codes[(zeros << 4) | size]);
  if (size > 0) {
    bits.write((value < 0 ? value + (1 << size) - 1 : value, size));
  }
}

/// The table's Huffman codes by symbol, as (code, length).
List<(int, int)> _codes(_Huffman table) {
  final codes = List<(int, int)>.filled(256, (0, 0));
  var code = 0;
  var k = 0;
  for (var length = 1; length <= 16; length++) {
    for (var n = 0; n < table.bits[length - 1]; n++) {
      codes[table.values[k++]] = (code++, length);
    }
    code <<= 1;
  }
  return codes;
}

/// Bits into bytes, with a zero byte after every 0xFF as the format requires.
class _Bits {
  _Bits(this.out);
  final BytesBuilder out;
  int _buffer = 0;
  int _count = 0;

  void write((int, int) code) {
    final (value, length) = code;
    _buffer = (_buffer << length) | (value & ((1 << length) - 1));
    _count += length;
    while (_count >= 8) {
      final byte = (_buffer >> (_count - 8)) & 0xFF;
      out.addByte(byte);
      if (byte == 0xFF) out.addByte(0);
      _count -= 8;
    }
    _buffer &= (1 << _count) - 1;
  }

  /// The last byte is filled with ones.
  void flush() {
    if (_count > 0) write(((1 << (8 - _count)) - 1, 8 - _count));
  }
}

/// A quantization table scaled for `quality` as the reference encoder does.
Int32List _scaled(List<int> base, int quality) {
  final q = quality.clamp(1, 100);
  final scale = q < 50 ? 5000 ~/ q : 200 - q * 2;
  return Int32List.fromList([
    for (final v in base) ((v * scale + 50) ~/ 100).clamp(1, 255),
  ]);
}

/// cos((2x + 1)uπ/16) with the DCT's factors, by u * 8 + x.
final _cos = Float64List.fromList([
  for (var u = 0; u < 8; u++)
    for (var x = 0; x < 8; x++)
      (u == 0 ? math.sqrt1_2 : 1) /
          2 *
          math.cos((2 * x + 1) * u * math.pi / 16),
]);

const _zigzag = [
  0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, //
  12, 19, 26, 33, 40, 48, 41, 34, 27, 20, 13, 6, 7, 14, 21, 28,
  35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51,
  58, 59, 52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];

const _lumaTable = [
  16, 11, 10, 16, 24, 40, 51, 61, //
  12, 12, 14, 19, 26, 58, 60, 55,
  14, 13, 16, 24, 40, 57, 69, 56,
  14, 17, 22, 29, 51, 87, 80, 62,
  18, 22, 37, 56, 68, 109, 103, 77,
  24, 35, 55, 64, 81, 104, 113, 92,
  49, 64, 78, 87, 103, 121, 120, 101,
  72, 92, 95, 98, 112, 100, 103, 99,
];

const _chromaTable = [
  17, 18, 24, 47, 99, 99, 99, 99, //
  18, 21, 26, 66, 99, 99, 99, 99,
  24, 26, 56, 99, 99, 99, 99, 99,
  47, 66, 99, 99, 99, 99, 99, 99,
  99, 99, 99, 99, 99, 99, 99, 99,
  99, 99, 99, 99, 99, 99, 99, 99,
  99, 99, 99, 99, 99, 99, 99, 99,
  99, 99, 99, 99, 99, 99, 99, 99,
];

/// A Huffman table as the format stores it: how many codes of each length 1 to 16, and
/// the symbols in order.
typedef _Huffman = ({List<int> bits, List<int> values});

const _Huffman _dcLuma = (
  bits: [0, 1, 5, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0],
  values: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
);

const _Huffman _dcChroma = (
  bits: [0, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0],
  values: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
);

const _Huffman _acLuma = (
  bits: [0, 2, 1, 3, 3, 2, 4, 3, 5, 5, 4, 4, 0, 0, 1, 0x7d],
  values: [
    0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05, 0x12, 0x21, 0x31, 0x41, 0x06, //
    0x13, 0x51, 0x61, 0x07, 0x22, 0x71, 0x14, 0x32, 0x81, 0x91, 0xa1, 0x08,
    0x23, 0x42, 0xb1, 0xc1, 0x15, 0x52, 0xd1, 0xf0, 0x24, 0x33, 0x62, 0x72,
    0x82, 0x09, 0x0a, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x25, 0x26, 0x27, 0x28,
    0x29, 0x2a, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x43, 0x44, 0x45,
    0x46, 0x47, 0x48, 0x49, 0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59,
    0x5a, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x6a, 0x73, 0x74, 0x75,
    0x76, 0x77, 0x78, 0x79, 0x7a, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89,
    0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0xa2, 0xa3,
    0xa4, 0xa5, 0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6,
    0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3, 0xc4, 0xc5, 0xc6, 0xc7, 0xc8, 0xc9,
    0xca, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda, 0xe1, 0xe2,
    0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea, 0xf1, 0xf2, 0xf3, 0xf4,
    0xf5, 0xf6, 0xf7, 0xf8, 0xf9, 0xfa,
  ],
);

const _Huffman _acChroma = (
  bits: [0, 2, 1, 2, 4, 4, 3, 4, 7, 5, 4, 4, 0, 1, 2, 0x77],
  values: [
    0x00, 0x01, 0x02, 0x03, 0x11, 0x04, 0x05, 0x21, 0x31, 0x06, 0x12, 0x41, //
    0x51, 0x07, 0x61, 0x71, 0x13, 0x22, 0x32, 0x81, 0x08, 0x14, 0x42, 0x91,
    0xa1, 0xb1, 0xc1, 0x09, 0x23, 0x33, 0x52, 0xf0, 0x15, 0x62, 0x72, 0xd1,
    0x0a, 0x16, 0x24, 0x34, 0xe1, 0x25, 0xf1, 0x17, 0x18, 0x19, 0x1a, 0x26,
    0x27, 0x28, 0x29, 0x2a, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x43, 0x44,
    0x45, 0x46, 0x47, 0x48, 0x49, 0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58,
    0x59, 0x5a, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x6a, 0x73, 0x74,
    0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87,
    0x88, 0x89, 0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a,
    0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4,
    0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3, 0xc4, 0xc5, 0xc6, 0xc7,
    0xc8, 0xc9, 0xca, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda,
    0xe2, 0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea, 0xf2, 0xf3, 0xf4,
    0xf5, 0xf6, 0xf7, 0xf8, 0xf9, 0xfa,
  ],
);
