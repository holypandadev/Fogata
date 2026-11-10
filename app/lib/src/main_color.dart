import 'dart:async';
import 'dart:collection';
import 'dart:typed_data';
import 'dart:ui' as ui;

import 'package:flutter/widgets.dart';

import 'rust/api/types.dart';
import 'safe_image.dart';
import 'theme.dart';

/// The side, in pixels, a picture is read at to find its main colour: enough to see
/// which colour covers most of it, and quick.
const _side = 24;

/// Most pictures whose main colour is kept, the last ones asked for.
const _kept = 64;

/// By the pictures' bytes, not their lists: the core hands over a new list with the same
/// bytes at every refresh.
final _colours = LinkedHashMap<Uint8List, Color?>(
  equals: samePicture,
  hashCode: pictureHash,
);

/// The colour that covers most of [bytes] (a photo): its pixels sorted into a few
/// thousand shades, each counted more the more vivid it is, so a small bright subject
/// on grey wins over the grey; the average of the shade that won. `null` when the
/// picture cannot be read, or has nothing but see-through pixels.
@visibleForTesting
Color? mainColourOf(Uint8List rgba) {
  final weights = <int, double>{};
  final sums = <int, List<double>>{};
  for (var i = 0; i + 3 < rgba.length; i += 4) {
    final r = rgba[i], g = rgba[i + 1], b = rgba[i + 2], a = rgba[i + 3];
    if (a < 128) continue;
    final shade = (r >> 4) << 8 | (g >> 4) << 4 | (b >> 4);
    final high = [r, g, b].reduce((x, y) => x > y ? x : y);
    final low = [r, g, b].reduce((x, y) => x < y ? x : y);
    final vivid = high == 0 ? 0.0 : (high - low) / high;
    final weight = 1 + 3 * vivid;
    weights[shade] = (weights[shade] ?? 0) + weight;
    final sum = sums.putIfAbsent(shade, () => [0, 0, 0, 0]);
    sum[0] += r;
    sum[1] += g;
    sum[2] += b;
    sum[3] += 1;
  }
  if (weights.isEmpty) return null;
  final best = weights.entries.reduce((x, y) => y.value > x.value ? y : x).key;
  final sum = sums[best]!;
  return Color.fromARGB(
    255,
    (sum[0] / sum[3]).round(),
    (sum[1] / sum[3]).round(),
    (sum[2] / sum[3]).round(),
  );
}

/// The main colour of [bytes], read once and kept for the last [_kept] pictures.
Future<Color?> mainColour(Uint8List bytes) async {
  if (_colours.containsKey(bytes)) return _colours[bytes];
  Color? colour;
  try {
    final key = SafeMemoryImage(bytes);
    final stream = ResizeImage(
      key,
      width: _side,
      height: _side,
      policy: ResizeImagePolicy.exact,
    ).resolve(ImageConfiguration.empty);
    final image = await _first(stream);
    if (image != null) {
      final data = await image.toByteData(format: ui.ImageByteFormat.rawRgba);
      image.dispose();
      if (data != null) colour = mainColourOf(data.buffer.asUint8List());
    }
  } catch (_) {
    colour = null;
  }
  _colours[bytes] = colour;
  if (_colours.length > _kept) _colours.remove(_colours.keys.first);
  return colour;
}

/// The first frame [stream] gives, or `null` when it cannot be read.
Future<ui.Image?> _first(ImageStream stream) {
  final done = Completer<ui.Image?>();
  late final ImageStreamListener listener;
  listener = ImageStreamListener(
    (info, _) {
      if (!done.isCompleted) done.complete(info.image.clone());
      info.dispose();
      stream.removeListener(listener);
    },
    onError: (_, _) {
      if (!done.isCompleted) done.complete(null);
      stream.removeListener(listener);
    },
  );
  stream.addListener(listener);
  return done.future;
}

/// Builds with the main colour of [avatar]: a photo's, once read (`null` until then), or
/// the colour of a preset avatar at once.
class MainColour extends StatefulWidget {
  const MainColour({super.key, required this.avatar, required this.builder});

  final UiAvatar avatar;
  final Widget Function(BuildContext context, Color? colour) builder;

  @override
  State<MainColour> createState() => _MainColourState();
}

class _MainColourState extends State<MainColour> {
  Color? _colour;
  Uint8List? _read;

  @override
  void initState() {
    super.initState();
    _update();
  }

  @override
  void didUpdateWidget(MainColour old) {
    super.didUpdateWidget(old);
    if (old.avatar != widget.avatar) _update();
  }

  void _update() {
    final photo = widget.avatar.photo;
    if (photo == null) {
      _read = null;
      _colour =
          avatarPresets[widget.avatar.preset % avatarPresets.length].color;
      return;
    }
    // The same photo in a new list, as each refresh brings it: nothing to read again.
    if (samePicture(photo, _read)) return;
    _read = photo;
    _colour = _colours[photo];
    mainColour(photo).then((colour) {
      if (mounted && identical(_read, photo)) setState(() => _colour = colour);
    });
  }

  @override
  Widget build(BuildContext context) => widget.builder(context, _colour);
}
