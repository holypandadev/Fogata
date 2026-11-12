// A group's activity: each line in words, what was undone, what only this device saw, and
// older lines a page at a time.

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/screens/activity.dart';
import 'package:fogata/src/theme.dart';

const ana = UiPerson(
  id: 'ana',
  name: 'Ana',
  short: '7Q4M·K2XR',
  avatar: UiAvatar(preset: 1),
);
const bea = UiPerson(
  id: 'bea',
  name: 'Bea',
  short: 'AB1C·D2EF',
  avatar: UiAvatar(preset: 2),
);
const leo = UiPerson(
  id: 'leo',
  name: 'Leo',
  short: 'AB3C·D4EF',
  avatar: UiAvatar(preset: 3),
);

UiActivity line(
  UiActivityKind kind, {
  UiPerson by = ana,
  UiPerson? member,
  List<UiPerson>? members,
  String? name,
  String? from,
  int? number,
  int? number2,
  int? untilMs,
  bool takenAway = false,
  int atMs = 1,
  UiUndoneKind? undone,
  UiPerson? undoneBy,
  bool seenHere = false,
}) => UiActivity(
  kind: kind,
  by: by,
  member: member,
  members: members,
  name: name,
  from: from,
  number: number,
  number2: number2,
  untilMs: untilMs,
  takenAway: takenAway,
  atMs: atMs,
  undone: undone,
  undoneBy: undoneBy,
  seenHere: seenHere,
);

const group = UiGroup(
  id: 'g',
  name: 'Sundays',
  localName: '',
  icon: 0,
  members: 3,
  online: 0,
  manager: true,
  rank: 1,
  outranks: ['leo'],
  maxMembers: 20,
  maxInRoom: 8,
  voiceRooms: [UiVoiceRoom(id: AppState.firstRoom, name: '')],
  openInvites: 0,
  needsUpdate: false,
);

void main() {
  test('each line reads as a sentence, this device as you', () {
    String say(UiActivity a) => activitySentence(a, 'bea');
    expect(
      say(line(UiActivityKind.created, name: 'Sundays')),
      'Ana started the group as “Sundays”',
    );
    expect(say(line(UiActivityKind.added, member: leo)), 'Ana added Leo');
    expect(say(line(UiActivityKind.added, member: bea)), 'Ana added you');
    expect(
      say(line(UiActivityKind.removed, by: bea, member: leo)),
      'You removed Leo',
    );
    expect(
      say(line(UiActivityKind.movedUp, member: leo, number: 1)),
      'Ana moved Leo up to 2nd',
    );
    expect(
      say(line(UiActivityKind.movedDown, member: leo)),
      'Ana moved Leo to the end',
    );
    expect(
      say(line(UiActivityKind.joined, by: leo, member: bea)),
      'Leo joined with an invite from you',
    );
    expect(say(line(UiActivityKind.letIn, member: leo)), 'Ana let Leo in');
    expect(say(line(UiActivityKind.left, by: leo)), 'Leo left the group');
    expect(
      say(line(UiActivityKind.managers)),
      'Ana let everyone manage the group',
    );
    expect(
      say(line(UiActivityKind.managers, number: 2)),
      'Ana left managing the group to the first 2',
    );
    expect(
      say(line(UiActivityKind.limits, number: 12, number2: 6)),
      'Ana set the group to 12 people and 6 in each voice room',
    );
    expect(
      say(line(UiActivityKind.roomRenamed, from: '', name: 'Games')),
      'Ana renamed the voice room “General” to “Games”',
    );
    expect(
      say(line(UiActivityKind.roomAccess, name: 'Games', members: [leo])),
      'Ana let only Leo into “Games”',
    );
    expect(
      say(line(UiActivityKind.roomAccess, name: 'Games')),
      'Ana opened “Games” to everyone',
    );
    expect(
      say(line(UiActivityKind.keptOut, member: leo, name: '')),
      'Ana kept Leo out of “General” until let back',
    );
    expect(
      say(line(UiActivityKind.picture, takenAway: true)),
      'Ana removed the group’s picture',
    );
    expect(undoneWords(line(UiActivityKind.renamed, name: 'x'), 'bea'), isNull);
    expect(
      undoneWords(
        line(
          UiActivityKind.removed,
          by: leo,
          member: ana,
          undone: UiUndoneKind.movedDown,
          undoneBy: bea,
        ),
        'bea',
      ),
      'Undone: Leo had been moved to the end by you at the same time',
    );
  });

  testWidgets(
    'the list shows what was undone, what this device saw, and older pages',
    (tester) async {
      final state = AppState('/nonexistent/fogata-activity-test')
        ..contacts.profile = const UiProfile(
          person: bea,
          fingerprint: 'AB1C D2EF',
        );
      // 120 lines in all: the newest a rename undone, then a banner seen here.
      final all = [
        line(
          UiActivityKind.renamed,
          by: leo,
          name: 'Taken',
          // Its author's clock is behind every other's.
          atMs: 0,
          undone: UiUndoneKind.movedDown,
          undoneBy: ana,
        ),
        line(UiActivityKind.banner, seenHere: true),
        for (var i = 0; i < 118; i++)
          line(UiActivityKind.renamed, name: 'Name $i', atMs: 1000 + i),
      ];
      final asked = <(int, int)>[];
      Future<UiActivityPage> load(String g, int skip, int take) async {
        asked.add((skip, take));
        final end = skip + take > all.length ? all.length : skip + take;
        return UiActivityPage(
          lines: all.sublist(skip, end),
          more: end < all.length,
        );
      }

      tester.view.physicalSize = const Size(900, 900);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      await tester.pumpWidget(
        AppScope(
          state: state,
          child: MaterialApp(
            theme: Fg.darkTokens.themeData,
            home: Material(
              child: SingleChildScrollView(
                child: GroupActivity(group: group, load: load),
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      expect(asked, [(0, activityPage)]);
      expect(
        find.text('In the group’s own order; times are each author’s clock.'),
        findsOneWidget,
      );
      // In the order given, whatever the times say.
      expect(
        tester.getTopLeft(find.text('Leo renamed the group “Taken”')).dy,
        lessThan(
          tester.getTopLeft(find.text('Ana renamed the group “Name 0”')).dy,
        ),
      );
      final undone = tester.widget<Text>(
        find.text('Leo renamed the group “Taken”'),
      );
      expect(undone.style?.decoration, TextDecoration.lineThrough);
      expect(
        find.text(
          'Undone: Leo had been moved to the end by Ana at the same time',
        ),
        findsOneWidget,
      );
      expect(find.textContaining('seen on this device'), findsOneWidget);
      expect(find.text('Ana changed the group’s banner'), findsOneWidget);
      expect(find.text('Ana renamed the group “Name 47”'), findsOneWidget);
      expect(find.text('Ana renamed the group “Name 48”'), findsNothing);
      await tester.ensureVisible(find.text('Show older'));
      await tester.tap(find.text('Show older'));
      await tester.pumpAndSettle();
      expect(asked.last, (activityPage, activityPage));
      expect(find.text('Ana renamed the group “Name 97”'), findsOneWidget);
      await tester.ensureVisible(find.text('Show older'));
      await tester.tap(find.text('Show older'));
      await tester.pumpAndSettle();
      expect(find.text('Ana renamed the group “Name 117”'), findsOneWidget);
      expect(find.text('Show older'), findsNothing);
    },
  );
}
