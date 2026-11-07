// A newer version that becomes ready is announced once, not at every word of the core.

import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/rust/api/update.dart' as api;
import 'package:fogata/src/update.dart';

api.UiUpdate _status(
  api.UiUpdateState state, {
  int? build,
  bool required = false,
}) => api.UiUpdate(
  thisBuild: 5,
  thisVersion: '0.1.0-beta.5',
  state: state,
  done: 0,
  total: 0,
  releaseBuild: build,
  releaseVersion: build == null ? null : '0.1.0-beta.$build',
  required_: required,
);

void main() {
  test('a version that becomes ready is announced once', () {
    final arrived = <int>[];
    final updates = Updates(
      onArrived: (status) => arrived.add(status.releaseBuild!.toInt()),
    );
    updates.take(_status(api.UiUpdateState.upToDate));
    updates.take(_status(api.UiUpdateState.checking));
    updates.take(_status(api.UiUpdateState.downloading, build: 7));
    expect(arrived, isEmpty);
    updates.take(_status(api.UiUpdateState.ready, build: 7));
    expect(arrived, [7]);
    // Installing, failing and being ready again is not news.
    updates.take(_status(api.UiUpdateState.installing, build: 7));
    updates.take(_status(api.UiUpdateState.ready, build: 7));
    expect(arrived, [7]);
    // A newer one is.
    updates.take(_status(api.UiUpdateState.ready, build: 8, required: true));
    expect(arrived, [7, 8]);
    expect(updates.ready, isTrue);
  });
}
