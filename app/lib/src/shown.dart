import 'package:flutter/foundation.dart';

/// Whether the window can be seen: false while it is hidden in the tray or minimized.
/// What only changes what is drawn (pulses, clocks, statistics read for the screen)
/// waits meanwhile.
final windowShown = ValueNotifier<bool>(true);
