import 'dart:ui';

import 'piece.dart';

enum Screen { home, friends, room, members, direct }

/// The parts of the settings, in the order their column lists them.
enum SettingsSection {
  profile,
  move,
  donate,
  appearance,
  languageTime,
  voice,
  sounds,
  behaviour,
  network,
  storage,
  report,
}

/// Where the window is: the page shown and the group selected, and what floats over it
/// (the settings, a group's settings, a friend's card, the person's panel).
class Navigation extends Piece {
  Screen page = Screen.home;

  /// The group selected in the rail; null for home.
  String? selectedGroup;

  /// The group whose members are shown.
  String? membersGroup;

  /// The page shown before a room or a call took the window, to go back to on leaving.
  ({Screen page, String? selected, String? members})? before;

  /// Settings float over the page instead of replacing it.
  bool settingsOpen = false;

  /// The part of the settings shown. None is the list of parts where the settings are too
  /// narrow to show the list beside a part, and My profile where they are not.
  SettingsSection? settingsSection;

  /// Asked before the settings close while they hold changes not saved: whether they
  /// may close (saved, or let go).
  Future<bool> Function()? settingsMayClose;

  /// The settings are asking [settingsMayClose] now.
  bool askingToClose = false;

  /// The settings open at the network part, once, when the room's network notice asked.
  bool networkCheckOnOpen = false;

  /// The group whose settings float over the window, and the tab they show.
  String? groupSettings;
  int groupSettingsTab = 0;

  /// The friend whose card floats next to [friendCardFrom], the bounds of what was clicked;
  /// null while no card is open. It may be a member of a group who is not a friend
  /// ([memberCard]): their card offers a friend request instead, and becomes a friend's
  /// card once they are one.
  String? friendCard;
  Rect? friendCardFrom;

  /// The card open is a group member's who was not a friend when it opened, and the group
  /// it was opened from, if one (its member list is looked in first).
  bool memberCard = false;
  String? memberCardGroup;

  /// What the person's panel opens from (the bounds of what was clicked); null while it
  /// is closed.
  Rect? userPanelFrom;

  /// The person's panel opened from their name or picture somewhere in the window, not
  /// from their own card at the bottom: it floats beside what was clicked, as a friend's
  /// card does.
  bool userPanelBeside = false;
}
