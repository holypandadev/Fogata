import 'package:flutter_rust_bridge/flutter_rust_bridge.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/format.dart';

void main() {
  test('known messages of the core read as sentences for people', () {
    expect(
      explain(AnyhowException('that voice room is full: 8 people at most')),
      'That voice room is full: it takes 8 people at most.',
    );
    expect(
      explain(AnyhowException('you are already in a call')),
      'You are already in a call. Hang up first.',
    );
    expect(
      explain('the link was already used or has expired; ask for a new one'),
      'That link was already used or has expired. Ask for a new one.',
    );
  });

  test('a panic of the core says only that something went wrong', () {
    for (final error in [
      PanicException('cannot open ‹path› for ‹peer 1›'),
      AnyhowException('a task panicked: no answer from ‹peer 1›'),
      'the file did not come: task 12 panicked with message "‹path›"',
      'the file did not come: an internal error stopped the work (the log says where)',
    ]) {
      expect(explain(error), 'Something went wrong.', reason: '$error');
    }
  });

  test('a full disk and a room with all the files one shares say so', () {
    expect(
      explain('No space left on device (os error 28)'),
      'The disk is full. Free some space and try again.',
    );
    expect(
      explain('There is not enough space on the disk. (os error 112)'),
      'The disk is full. Free some space and try again.',
    );
    expect(
      explain(
        AnyhowException(
          'you share 100 files in this room already: stop sharing some first',
        ),
      ),
      'You already share the most files one person can in a room. Stop sharing some first.',
    );
  });

  test('a device that wrote too much for its run is told what to wait for', () {
    expect(
      explain(
        AnyhowException(
          'not allowed: its author wrote too much that no other member signed on yet',
        ),
      ),
      'This device has made too many changes to the group that no other member has taken in yet. Try again after someone else in the group makes a change.',
    );
  });

  test(
    'why someone let into a group did not get in, and a file with no room',
    () {
      for (final (core, said) in [
        (
          'that invite no longer works, or the group is full',
          'That invite no longer works, or the group is full.',
        ),
        (
          'this device has no group key yet: the new member gets in once it has one',
          'This device doesn’t have the group’s key yet. The new member gets in as soon as it does.',
        ),
        (
          'this device wrote more than the others have seen yet: the new member can be let in '
              'once another member is online',
          'This device has made changes the other members haven’t taken in yet. The new member '
              'can be let in once another member is online.',
        ),
        (
          "that device's request was not made on this group as it is: it has to ask again",
          'That request to join was made on an earlier state of the group. The person has to '
              'ask again.',
        ),
      ]) {
        expect(explain(AnyhowException(core)), said);
        expect(notLetIn(core), said);
      }
      expect(notLetIn('a new member joined with an invite (Ab12Cd)'), isNull);
      expect(
        explain(AnyhowException('not enough free space for this file')),
        'There isn’t enough free space for this file. Free some space and try again.',
      );
    },
  );

  test('anything else starts with a capital and ends with a period', () {
    expect(explain(AnyhowException('the disk is full')), 'The disk is full.');
    expect(explain('Already a sentence.'), 'Already a sentence.');
    expect(explain(''), 'Something went wrong.');
  });
}
