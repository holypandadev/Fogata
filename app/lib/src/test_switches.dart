import 'dart:io';

import 'package:flutter/foundation.dart';

/// The value of [name], a variable the application's own tests set to change what it
/// does (`FOGATA_TEST_OPENED`, `FOGATA_DOWNLOADS`, `FLUTTER_TEST`), or null when it is
/// not set. A release build reads none of them: whatever starts it (a launcher, a login
/// item, an edited shortcut) must not make links and folders open nothing, or move where
/// files are saved. Debug and profile builds, which the tests and the call lab run, read
/// them as before.
///
/// The variables a release build does read are read where they are used: the data folder
/// (`FOGATA_DATA_DIR`), the call's steps on standard error (`FOGATA_CALL_LOG`) and the
/// system's own.
String? testSwitch(
  String name, {
  @visibleForTesting bool release = kReleaseMode,
  @visibleForTesting Map<String, String>? environment,
}) => release ? null : (environment ?? Platform.environment)[name];
