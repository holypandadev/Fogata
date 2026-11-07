import 'dart:io';

import 'package:flutter/foundation.dart';

/// The desktop systems Fogata runs on.
enum HostSystem { linux, macos, windows }

/// The system this copy runs on, as the interface follows it: its fonts, its title bar, its
/// keyboard shortcuts, the words that name the system's own settings, and which of the
/// video library's ways it takes. Code that reaches the system itself (files and paths,
/// processes, libraries) asks `dart:io`'s [Platform] instead.
HostSystem get host => _pinned ?? _running;

final _running = Platform.isMacOS
    ? HostSystem.macos
    : Platform.isWindows
    ? HostSystem.windows
    : HostSystem.linux;

HostSystem? _pinned;

/// Makes [host] say `system` (null: the one running): the widget tests pin it
/// (test/flutter_test_config.dart), so that they measure the same on every computer that
/// runs them, and tests of another system's way set it for a while.
@visibleForTesting
set pinnedHost(HostSystem? system) => _pinned = system;

bool get onLinux => host == HostSystem.linux;
bool get onMacOS => host == HostSystem.macos;
bool get onWindows => host == HostSystem.windows;
