import 'package:flutter/foundation.dart';

import 'rust/api/app.dart' as api;

/// Writes [bytes] to [path] as the core writes its own files: readable by the owner only
/// (on Unix), through a temporary file, so a crash leaves the old contents or the new
/// ones, and a link at [path] is replaced rather than written through. Dart itself makes
/// every file readable by everyone the umask lets in. The folder must be there.
Future<void> writePrivate(String path, List<int> bytes) =>
    privateWriter(path, bytes is Uint8List ? bytes : Uint8List.fromList(bytes));

/// How [writePrivate] writes: through the core's library. Tests, which run without it,
/// put a writer of their own.
@visibleForTesting
Future<void> Function(String path, Uint8List bytes) privateWriter = (
  path,
  bytes,
) => api.writePrivate(path: path, bytes: bytes);
