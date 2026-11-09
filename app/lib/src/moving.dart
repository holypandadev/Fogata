import 'dart:async';
import 'dart:ui' as ui;

import 'package:file_selector/file_selector.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';

import 'l10n.dart';
import 'photo.dart';
import 'round.dart';
import 'safe_image.dart';
import 'shown.dart';

export 'photo.dart' show PictureProblem;

// Moving pictures: a GIF or WebP animation shown in place of a profile picture or a card's
// banner. Each still picture (a profile's, a banner's) is the animation's first frame, the
// one every version of the app shows and the one shown while an animation waits. An
// animation plays only while it is on screen and the window is seen and focused; at any
// other time it stays on the frame it reached.

/// Most bytes of an animated profile picture: it goes to every member of every group, at
/// each connection.
const maxMovingPhotoBytes = 550 * 1024;

/// Most bytes of an animated banner, a friend's card's or a group's.
const maxMovingBannerBytes = 550 * 1024;

/// Avatars smaller than this never move: the lists' small pictures stay still.
const minMovingAvatar = 48.0;

/// Most pixels of an animated profile picture. Every frame is decoded at the animation's
/// full size however small it is drawn, again each time it comes round, so the size is
/// held to about what the largest avatar needs; the core refuses larger ones too.
const maxMovingPhotoPixels = 512 * 512;

/// Most pixels of an animated banner of a person's card (three times as wide as tall).
const maxMovingBannerPixels = 1024 * 384;

/// Most pixels of an animated banner of a group (16 by 9).
const maxMovingGroupBannerPixels = 1024 * 576;

/// Most frames of an animation; the core refuses more too.
const maxMovingFrames = 500;

/// Most animations playing at once in the window; the others stay on the frame they
/// reached (the first, for one that never got to play) until one of those playing stops.
const maxPlayingAnimations = 6;

/// Whether [bytes] are a GIF or a WebP, the two kinds of animation the app plays.
bool isMovingKind(Uint8List bytes) {
  bool at(int from, String text) =>
      bytes.length >= from + text.length &&
      String.fromCharCodes(bytes.sublist(from, from + text.length)) == text;
  return at(0, 'GIF87a') || at(0, 'GIF89a') || (at(0, 'RIFF') && at(8, 'WEBP'));
}

/// How many frames the picture has, or 0 when it cannot be read.
Future<int> frameCount(Uint8List bytes) async =>
    (await movingSize(bytes))?.frames ?? 0;

/// The size of a picture and how many frames it has, read from its header; counting the
/// frames decodes none of them. Null when it cannot be read.
Future<({int width, int height, int frames})?> movingSize(
  Uint8List bytes,
) async {
  final buffer = await ui.ImmutableBuffer.fromUint8List(bytes);
  ui.ImageDescriptor? descriptor;
  ui.Codec? codec;
  try {
    descriptor = await ui.ImageDescriptor.encoded(buffer);
    if (descriptor.width <= 0 || descriptor.height <= 0) return null;
    codec = await descriptor.instantiateCodec();
    return (
      width: descriptor.width,
      height: descriptor.height,
      frames: codec.frameCount,
    );
  } catch (_) {
    return null;
  } finally {
    codec?.dispose();
    descriptor?.dispose();
    buffer.dispose();
  }
}

/// Most pixels of an animation shown as [kind] ('pictures', 'banners' or 'groupBanners'),
/// with the size that says it.
({int pixels, String size}) _most(String kind) => switch (kind) {
  'pictures' => (pixels: maxMovingPhotoPixels, size: _size(512, 512)),
  'banners' => (pixels: maxMovingBannerPixels, size: _size(1024, 384)),
  _ => (pixels: maxMovingGroupBannerPixels, size: _size(1024, 576)),
};

/// A size in pixels, kept on one line.
String _size(int width, int height) => '$width\u00a0×\u00a0$height';

String _kb(int bytes) => l10n.sizeKB('${(bytes / 1024).ceil()}');

/// A picture picked from the computer: its still form for every version of the app, and
/// the animation when it is a GIF or WebP of more than one frame.
typedef Picked = ({Uint8List still, Uint8List? moving});

/// Lets the person pick a picture for their profile: a photo becomes a small square
/// (photo.dart); a GIF or WebP that moves is kept as it is, at most [maxMovingPhotoBytes],
/// with its first frame as the still picture. Null when nothing was picked; throws
/// [PictureProblem] when the picture cannot be used.
Future<Picked?> pickProfilePicture() async {
  final file = await openFile(acceptedTypeGroups: [_pictures]);
  if (file == null) return null;
  final bytes = await readPicked(file);
  final moving = await _moving(bytes, maxMovingPhotoBytes, 'pictures');
  final still = await squarePicture(bytes);
  if (still == null) {
    throw PictureProblem(unreadablePicture);
  }
  return (still: still, moving: moving);
}

/// Lets the person pick a group's picture: a photo becomes a small square of at most
/// [maxGroupPictureBytes] (photo.dart); a GIF or WebP that moves stays as it is, at most
/// [maxMovingPhotoBytes], and its first frame shows wherever it does not play. Null when
/// nothing was picked; throws [PictureProblem] when the picture cannot be used.
Future<Uint8List?> pickGroupPhoto() async {
  final file = await openFile(acceptedTypeGroups: [_pictures]);
  if (file == null) return null;
  final bytes = await readPicked(file);
  final moving = await _moving(bytes, maxMovingPhotoBytes, 'pictures');
  if (moving != null) return moving;
  final still = await squarePicture(bytes, maxBytes: maxGroupPictureBytes);
  if (still == null) {
    throw PictureProblem(unreadablePicture);
  }
  return still;
}

/// Lets the person pick a picture for the top of their card: [bannerPng] of it, and the
/// animation as it is when it moves (at most [maxMovingBannerBytes]).
Future<Picked?> pickCardBanner() async {
  final file = await openFile(acceptedTypeGroups: [_pictures]);
  if (file == null) return null;
  final bytes = await readPicked(file);
  final moving = await _moving(bytes, maxMovingBannerBytes, 'banners');
  final still = await bannerPng(bytes);
  if (still == null) {
    throw PictureProblem(unreadablePicture);
  }
  return (still: still, moving: moving);
}

/// Lets the person pick a group's banner: a GIF or WebP that moves stays as it is (at most
/// [maxMovingBannerBytes]); a still picture is cut to fit ([groupBannerPicture]).
Future<Uint8List?> pickGroupBanner() async {
  final file = await openFile(acceptedTypeGroups: [_pictures]);
  if (file == null) return null;
  return groupBannerOf(await readPicked(file));
}

/// [pickGroupBanner] for bytes already read.
Future<Uint8List> groupBannerOf(Uint8List bytes) async {
  final moving = await _moving(bytes, maxMovingBannerBytes, 'groupBanners');
  if (moving != null) return moving;
  final still = await groupBannerPicture(bytes);
  if (still == null) {
    throw PictureProblem(unreadablePicture);
  }
  return still;
}

XTypeGroup get _pictures => XTypeGroup(
  label: l10n.photoImages,
  extensions: const ['png', 'jpg', 'jpeg', 'webp', 'gif'],
);

/// [bytes] when they are an animation that fits in [maxBytes] and in what animations shown
/// as [kind] may have (see [_most], [maxMovingFrames]); null for a still picture.
Future<Uint8List?> _moving(Uint8List bytes, int maxBytes, String kind) async {
  if (!isMovingKind(bytes)) return null;
  final size = await movingSize(bytes);
  if (size == null) throw PictureProblem(unreadablePicture);
  if (size.frames < 2) return null;
  final what = kind == 'pictures' ? 'pictures' : 'banners';
  final most = _most(kind);
  if (size.width * size.height > most.pixels) {
    throw PictureProblem(
      l10n.movingTooLarge(what, most.size, _size(size.width, size.height)),
    );
  }
  if (size.frames > maxMovingFrames) {
    throw PictureProblem(
      l10n.movingTooLong(what, '$maxMovingFrames', '${size.frames}'),
    );
  }
  if (bytes.length > maxBytes) {
    throw PictureProblem(
      l10n.movingTooHeavy(what, _kb(maxBytes), _kb(bytes.length)),
    );
  }
  return bytes;
}

/// Whether the window is focused: an animation stops when another window is.
final windowFocused = ValueNotifier<bool>(true);

/// Whether animations may play now: the window is seen and focused.
final motionAllowed = _Both(windowShown, windowFocused);

class _Both extends ChangeNotifier implements ValueListenable<bool> {
  _Both(this.a, this.b) {
    a.addListener(_changed);
    b.addListener(_changed);
  }

  final ValueListenable<bool> a, b;
  late bool _value = a.value && b.value;

  void _changed() {
    final now = a.value && b.value;
    if (now == _value) return;
    _value = now;
    notifyListeners();
  }

  @override
  bool get value => _value;
}

/// An animation known for a person: the still picture it moves in place of, and itself.
typedef Moving = ({Uint8List still, Uint8List picture});

/// The animations of other people's pictures and banners, asked for as their pictures are
/// first shown large, and this device's own. Each kind keeps the people shown last, at most
/// [kept]: one shown again after it went is asked for again.
class MovingBook extends ChangeNotifier {
  MovingBook({this.kept = 48});

  /// Most people whose picture's (and whose banner's) animation is kept.
  final int kept;

  /// In the order last shown, the latest last.
  final _photos = <String, _Asked>{};
  final _banners = <String, _Asked>{};

  /// Asks for a person's animation (`banner`: of the banner, else of the picture); it is
  /// put in with [put] once there. Set by the application.
  void Function(String id, {required bool banner})? ask;

  /// The animation of [id]'s picture, when it moves in place of [still].
  Uint8List? photo(String id, Uint8List? still) =>
      _find(_photos, id, still, banner: false);

  /// The animation of [id]'s banner, when it moves in place of [still].
  Uint8List? banner(String id, Uint8List? still) =>
      _find(_banners, id, still, banner: true);

  Uint8List? _find(
    Map<String, _Asked> book,
    String id,
    Uint8List? still, {
    required bool banner,
  }) {
    if (still == null) return null;
    final asked = book.remove(id);
    if (asked != null) book[id] = asked;
    // Asked once for each still picture: the animation and the picture it moves in place
    // of come apart, either first.
    if (asked == null || !samePicture(asked.still, still)) {
      book[id] = _Asked(still, asked?.got);
      while (book.length > kept) {
        book.remove(book.keys.first);
      }
      ask?.call(id, banner: banner);
    }
    final moving = book[id]!.got;
    if (moving == null || !samePicture(moving.still, still)) return null;
    return moving.picture;
  }

  /// What came for [id]: an animation, or null for none.
  void put(String id, Moving? moving, {required bool banner}) {
    final book = banner ? _banners : _photos;
    final asked = book[id];
    if (asked == null) return;
    if (asked.got == null && moving == null) return;
    asked.got = moving;
    notifyListeners();
  }

  /// [id]'s animations changed: they are asked for again when next shown.
  void forget(String id) {
    final photo = _photos.remove(id);
    final banner = _banners.remove(id);
    if (photo != null || banner != null) notifyListeners();
  }

  /// Everything, when the application starts again with another profile.
  void clear() {
    _photos.clear();
    _banners.clear();
    notifyListeners();
  }
}

/// The still picture an animation was asked for with, and the answer.
class _Asked {
  _Asked(this.still, this.got);
  final Uint8List still;
  Moving? got;
}

/// Every animation the window knows of.
final movingBook = MovingBook();

/// How many animations' frames are held, freed or not yet.
@visibleForTesting
int get liveMovingFrames => _Frames._live;

/// How many animations play now (see [maxPlayingAnimations]).
@visibleForTesting
int get playingAnimations => _Frames._players.length;

/// Frames of an animation, decoded about [width] by [height] pixels (the whole picture
/// covering that box), stepping while [play] says so and it has one of the
/// [maxPlayingAnimations] turns to play. An animation of more than [maxPixels] or
/// [maxMovingFrames] is not played.
class _Frames extends ChangeNotifier {
  _Frames(this.bytes, this.width, this.height, this.maxPixels) {
    _live++;
  }

  static int _live = 0;

  /// Those playing now, and those waiting for a turn, first come first.
  static final _players = <_Frames>{};
  static final _waiting = <_Frames>{};

  final Uint8List bytes;
  final int width, height, maxPixels;

  // The codec reads the encoded picture each time it decodes a frame, so the buffer and
  // descriptor stay as long as it does, and none of the three goes while a frame is being
  // decoded: freed as soon as the codec was made, the engine crashed on the first frame.
  ui.ImmutableBuffer? _buffer;
  ui.ImageDescriptor? _descriptor;
  ui.Codec? _codec;
  ui.Image? image;
  Timer? _timer;

  /// [play] says it should play, and it plays now (it has a turn).
  bool _wanted = false;
  bool _playing = false;
  bool _opening = false;
  bool _stepping = false;
  bool _disposed = false;
  bool _single = false;

  /// It cannot be played: it takes no turn.
  bool _unplayable = false;

  /// Reads the picture and shows its first frame; false if it cannot be played.
  Future<bool> open() async {
    _opening = true;
    var ok = false;
    try {
      if (!await fitsToDecode(bytes, maxPixels) || _disposed) {
        return false;
      }
      final buffer = _buffer = await ui.ImmutableBuffer.fromUint8List(bytes);
      final descriptor = _descriptor = await ui.ImageDescriptor.encoded(buffer);
      // As large as needed to cover the box, never larger than the picture.
      final cover = [
        width / descriptor.width,
        height / descriptor.height,
      ].reduce((a, b) => a > b ? a : b);
      final scale = cover > 1 ? 1.0 : cover;
      final codec = _codec = await descriptor.instantiateCodec(
        targetWidth: (descriptor.width * scale).ceil(),
        targetHeight: (descriptor.height * scale).ceil(),
      );
      if (_disposed || codec.frameCount > maxMovingFrames) return false;
      _single = codec.frameCount < 2;
      // One frame needs no turn to play.
      if (_single) _leave();
      await _step(schedule: false);
      return ok = image != null;
    } catch (_) {
      return false;
    } finally {
      _opening = false;
      if (!ok) {
        _unplayable = true;
        _leave();
      }
      if (_disposed) _release();
    }
  }

  set play(bool on) {
    if (on == _wanted) return;
    _wanted = on;
    if (!on) {
      _leave();
    } else if (!_single && !_unplayable) {
      _waiting.add(this);
      _takeTurns();
    }
  }

  /// Gives the turns that are free to those waiting, first come first.
  static void _takeTurns() {
    while (_players.length < maxPlayingAnimations && _waiting.isNotEmpty) {
      final next = _waiting.first;
      _waiting.remove(next);
      _players.add(next);
      next._playing = true;
      next._step();
    }
  }

  /// Stops playing, or waiting to, and lets the next one waiting play.
  void _leave() {
    _waiting.remove(this);
    _playing = false;
    _timer?.cancel();
    _timer = null;
    if (_players.remove(this)) _takeTurns();
  }

  Future<void> _step({bool schedule = true}) async {
    final codec = _codec;
    if (codec == null || _stepping || _disposed) return;
    if (schedule && _single) return;
    _stepping = true;
    try {
      final frame = await codec.getNextFrame();
      if (_disposed) {
        frame.image.dispose();
        return;
      }
      image?.dispose();
      image = frame.image;
      notifyListeners();
      if (_playing && !_single) {
        // Browsers draw frames said to last 10 ms or less at 100 ms; so does this.
        var wait = frame.duration;
        if (wait <= const Duration(milliseconds: 10)) {
          wait = const Duration(milliseconds: 100);
        }
        _timer?.cancel();
        _timer = Timer(wait, () {
          _timer = null;
          if (_playing) _step();
        });
      }
    } catch (_) {
      // A damaged frame: the one shown stays.
    } finally {
      _stepping = false;
      if (_disposed && !_opening) _release();
    }
  }

  /// Safe to call more than once.
  @override
  void dispose() {
    if (_disposed) return;
    _disposed = true;
    _live--;
    _leave();
    if (!_opening && !_stepping) _release();
    image?.dispose();
    image = null;
    super.dispose();
  }

  /// Frees the codec and what it reads, once no frame is being decoded.
  void _release() {
    _codec?.dispose();
    _codec = null;
    _descriptor?.dispose();
    _descriptor = null;
    _buffer?.dispose();
    _buffer = null;
  }
}

/// Plays [bytes] in a box of [size], filling it as [BoxFit.cover] does; [round] cuts it to
/// a circle with a smooth edge, and [corner] (of the box's shorter side) to a square with
/// round corners. [still] shows until the first frame is there, and for good when the
/// animation cannot be played.
class MovingPicture extends StatefulWidget {
  const MovingPicture({
    super.key,
    required this.bytes,
    required this.still,
    required this.maxPixels,
    this.round = false,
    this.corner,
    this.under,
  });

  final Uint8List bytes;
  final Widget still;
  final bool round;

  /// The corners' radius, as a part of the box's shorter side; ignored when [round].
  final double? corner;

  /// Most pixels it may have to play: [maxMovingPhotoPixels], [maxMovingBannerPixels] or
  /// [maxMovingGroupBannerPixels], by what it moves in place of.
  final int maxPixels;

  /// Drawn under the frames' see-through parts.
  final Color? under;

  @override
  State<MovingPicture> createState() => _MovingPictureState();
}

class _MovingPictureState extends State<MovingPicture> {
  _Frames? _frames;
  Size _box = Size.zero;
  bool _failed = false;

  void _load(Size box, double scale) {
    final width = (box.width * scale).ceil();
    final height = (box.height * scale).ceil();
    final old = _frames;
    _box = box;
    // Frames decoded for a box up to twice as small still look sharp enough, and decoding
    // again starts the animation over from its first frame: a window resized step by
    // step would keep it there.
    if (old != null &&
        old.bytes == widget.bytes &&
        width <= old.width * 2 &&
        height <= old.height * 2) {
      return;
    }
    final frames = _Frames(widget.bytes, width, height, widget.maxPixels);
    frames.open().then((ok) {
      // The frames this load replaces go once it is ready; frames whose load was
      // replaced in the meantime, or that outlived the widget, go too.
      old?.dispose();
      if (!mounted || _frames != frames) {
        frames.dispose();
        return;
      }
      setState(() {
        _failed = !ok;
        _update();
      });
    });
    _frames = frames;
    _failed = false;
  }

  void _update() => _frames?.play =
      motionAllowed.value && TickerMode.valuesOf(context).enabled;

  @override
  void initState() {
    super.initState();
    motionAllowed.addListener(_motion);
  }

  void _motion() {
    if (mounted) _update();
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _update();
  }

  @override
  void didUpdateWidget(MovingPicture old) {
    super.didUpdateWidget(old);
    if (old.bytes != widget.bytes) _box = Size.zero;
  }

  @override
  void dispose() {
    motionAllowed.removeListener(_motion);
    _frames?.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => LayoutBuilder(
    builder: (context, constraints) {
      final box = constraints.biggest;
      if (box != _box && box.isFinite && !box.isEmpty) {
        _load(box, MediaQuery.devicePixelRatioOf(context));
      }
      final frames = _frames;
      if (_failed || frames == null || frames.image == null) {
        return widget.still;
      }
      // Only this picture is drawn again at each frame.
      return RepaintBoundary(
        child: CustomPaint(
          size: box,
          painter: _FramePainter(
            frames,
            round: widget.round,
            corner: widget.corner,
            under: widget.under,
            scale: MediaQuery.devicePixelRatioOf(context),
          ),
        ),
      );
    },
  );
}

/// A picture someone sent to a chat. [still] draws it, its first frame only (see
/// [SafeMemoryImage]), and gives it its size; a GIF or WebP of more than one frame plays in
/// its place by the rules of every animation here: at most [maxAnimationPixels] and
/// [maxMovingFrames], decoded at the size drawn, while seen and focused, and in turns.
class ChatPicture extends StatelessWidget {
  const ChatPicture({super.key, required this.bytes, required this.still});

  final Uint8List bytes;
  final Widget still;

  @override
  Widget build(BuildContext context) {
    if (!isMovingKind(bytes)) return still;
    return Stack(
      fit: StackFit.passthrough,
      children: [
        Visibility(
          visible: false,
          maintainState: true,
          maintainAnimation: true,
          maintainSize: true,
          child: still,
        ),
        Positioned.fill(
          child: MovingPicture(
            bytes: bytes,
            still: still,
            maxPixels: maxAnimationPixels,
          ),
        ),
      ],
    );
  }
}

class _FramePainter extends CustomPainter {
  _FramePainter(
    this.frames, {
    required this.round,
    required this.corner,
    required this.under,
    required this.scale,
  }) : super(repaint: Listenable.merge([frames, Discs.ready]));

  final _Frames frames;
  final bool round;
  final double? corner;
  final Color? under;
  final double scale;

  @override
  void paint(Canvas canvas, Size size) {
    final image = frames.image;
    if (image == null) return;
    final box = Offset.zero & size;
    final source = Size(image.width.toDouble(), image.height.toDouble());
    final fitted = applyBoxFit(BoxFit.cover, source, size);
    final from = Alignment.center.inscribe(fitted.source, Offset.zero & source);
    final paint = Paint()..filterQuality = FilterQuality.medium;
    final under = this.under;
    final corner = this.corner;
    if (!round && corner != null && corner > 0) {
      // Cut along the round corners, smoothed.
      canvas.save();
      canvas.clipRRect(
        RRect.fromRectAndRadius(
          box,
          Radius.circular(corner.clamp(0, .5) * size.shortestSide),
        ),
      );
      if (under != null) canvas.drawRect(box, Paint()..color = under);
      canvas.drawImageRect(image, from, box, paint);
      canvas.restore();
      return;
    }
    if (!round) {
      if (under != null) canvas.drawRect(box, Paint()..color = under);
      canvas.drawImageRect(image, from, box, paint);
      return;
    }
    // The frame, then a disc that keeps only what it covers: an edge as smooth as the
    // still pictures' (round.dart), without cutting each frame's pixels.
    canvas.saveLayer(box.inflate(2), Paint());
    if (under != null) canvas.drawRect(box, Paint()..color = under);
    canvas.drawImageRect(image, from, box, paint);
    Discs.paintMask(canvas, box, scale);
    canvas.restore();
  }

  @override
  bool shouldRepaint(_FramePainter old) =>
      old.frames != frames ||
      old.round != round ||
      old.corner != corner ||
      old.under != under ||
      old.scale != scale;
}
