import 'dart:ffi';
import 'dart:io';

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';

import 'moving.dart' show windowFocused;
import 'test_switches.dart';

// Less motion, for the people who asked their system for it (macOS's Reduce motion,
// Windows' "Animation effects" off, GNOME's Animations off): the springs land at once
// without bouncing, and what moves for as long as it shows (the rings of a call, the
// talking bars, the typing dots, a placeholder's shimmer, moving pictures) holds still.
// Fades stay. Nothing changes for anyone else.
//
// Flutter is told of it on Linux (GTK's enable-animations), not on macOS or Windows:
// there the runner's channel (macOS) or the system's own setting (Windows) is read too.

bool _less = false;

/// Whether what [context] draws moves less: the system asked for less motion
/// ([MediaQuery.disableAnimations], which [FollowMotion] sets where Flutter is not told).
bool lessMotion(BuildContext context) =>
    (MediaQuery.maybeDisableAnimationsOf(context) ?? false) || _less;

/// [curve], or, with less motion asked for, already at its end: what it moves is where it
/// goes at once. Animations that run backwards stay at their start until they end, which
/// leaves a fade beside it as the only change.
class MotionCurve extends Curve {
  const MotionCurve(this.curve);
  final Curve curve;

  @override
  double transformInternal(double t) => _less ? 1 : curve.transform(t);
}

/// Above everything the window shows: keeps [lessMotion] and [MotionCurve] to what the
/// system says, and gives it to [MediaQuery.disableAnimations] where Flutter does not.
class FollowMotion extends StatefulWidget {
  const FollowMotion({super.key, required this.child});
  final Widget child;

  @override
  State<FollowMotion> createState() => _FollowMotionState();
}

class _FollowMotionState extends State<FollowMotion> {
  static const _channel = MethodChannel('fogata/motion');

  /// What macOS or Windows says, where Flutter is not told.
  bool _system = false;

  @override
  void initState() {
    super.initState();
    if (Platform.isMacOS) {
      _channel.setMethodCallHandler((call) async {
        if (call.method == 'reduceMotion' && call.arguments is bool) {
          _set(call.arguments as bool);
        }
      });
      _channel
          .invokeMethod<bool>('reduceMotion')
          .then((on) => _set(on ?? false), onError: (_) {});
    } else if (Platform.isWindows) {
      _readWindows();
      // Windows says nothing when it changes: read again as the window comes back.
      windowFocused.addListener(_readWindows);
    }
  }

  @override
  void dispose() {
    if (Platform.isMacOS) _channel.setMethodCallHandler(null);
    windowFocused.removeListener(_readWindows);
    super.dispose();
  }

  void _readWindows() {
    if (windowFocused.value) _set(windowsAsksLessMotion());
  }

  void _set(bool on) {
    if (mounted && on != _system) setState(() => _system = on);
  }

  @override
  Widget build(BuildContext context) {
    final data = MediaQuery.of(context);
    final less = data.disableAnimations || _system;
    // Read by the curves as they run, which have no context.
    _less = less;
    if (less == data.disableAnimations) return widget.child;
    return MediaQuery(
      data: data.copyWith(disableAnimations: less),
      child: widget.child,
    );
  }
}

/// What Windows says of its "Animation effects" ([windowsAnimationsOff]), but for the
/// application's own tests, which must see the same on every computer: a server, as the
/// ones that run them, has those effects off, and every spring there landed at once.
@visibleForTesting
bool Function() windowsAsksLessMotion = testSwitch('FLUTTER_TEST') != null
    ? () => false
    : windowsAnimationsOff;

/// Whether Windows' "Animation effects" are off (`SPI_GETCLIENTAREAANIMATION`); false
/// when it cannot be read.
bool windowsAnimationsOff() {
  try {
    final user32 = DynamicLibrary.open('user32.dll');
    final kernel32 = DynamicLibrary.open('kernel32.dll');
    final systemParametersInfo = user32
        .lookupFunction<
          Int32 Function(Uint32, Uint32, Pointer<Int32>, Uint32),
          int Function(int, int, Pointer<Int32>, int)
        >('SystemParametersInfoW');
    final localAlloc = kernel32
        .lookupFunction<
          Pointer<Int32> Function(Uint32, IntPtr),
          Pointer<Int32> Function(int, int)
        >('LocalAlloc');
    final localFree = kernel32
        .lookupFunction<
          Pointer<Void> Function(Pointer<Int32>),
          Pointer<Void> Function(Pointer<Int32>)
        >('LocalFree');
    const lptr = 0x40; // LMEM_FIXED | LMEM_ZEROINIT
    const getClientAreaAnimation = 0x1042;
    final on = localAlloc(lptr, sizeOf<Int32>());
    if (on == nullptr) return false;
    try {
      // A BOOL: whether animations of the interface are on.
      if (systemParametersInfo(getClientAreaAnimation, 0, on, 0) == 0) {
        return false;
      }
      return on.value == 0;
    } finally {
      localFree(on);
    }
  } catch (_) {
    return false;
  }
}

/// For tests: as when the system asks for less motion, without a [FollowMotion].
@visibleForTesting
set lessMotionForTests(bool on) => _less = on;
