// A video someone else shares in a room, opened to play: its fetch shows as it goes.

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/screens/room.dart';
import 'package:fogata/src/theme.dart';

import 'fake_room.dart';

/// A state whose fetch to play never ends, as a slow one.
class _Fetching extends AppState {
  _Fetching() : super('/nonexistent/fogata-room-video');

  @override
  Future<String?> videoToPlay(UiFile f) => Completer<String?>().future;
}

UiRoom _room(int got) => UiRoom(
  startedMs: 0,
  joined: true,
  participants: const [],
  chat: const [],
  files: [
    UiFile(
      id: 'v',
      name: 'trip.mp4',
      size: 1000,
      sharedBy: other(0),
      holders: [other(0)],
      mine: false,
      downloading: got,
      sent: 0,
    ),
  ],
);

void main() {
  testWidgets('getting a video shows how far it got as it grows', (
    tester,
  ) async {
    final state = _Fetching()
      ..contacts.profile = const UiProfile(person: me, fingerprint: '7Q4M K2XR')
      ..roomState.roomGroup = 'g'
      ..roomState.room = _room(100);
    late BuildContext context;
    await tester.pumpWidget(
      AppScope(
        state: state,
        child: MaterialApp(
          theme: Fg.darkTokens.themeData,
          home: Builder(
            builder: (c) {
              context = c;
              return const SizedBox();
            },
          ),
        ),
      ),
    );
    showRoomVideo(
      context,
      state,
      SeenFile('v', 'trip.mp4', 1000, other(0).id, other(0).name),
    );
    await tester.pump(const Duration(seconds: 1));
    expect(find.text('Getting the video · 10%'), findsOneWidget);
    state.roomState.room = _room(500);
    state.notifyListeners();
    await tester.pump();
    expect(find.text('Getting the video · 50%'), findsOneWidget);
    await tester.pumpWidget(const SizedBox());
  });
}
