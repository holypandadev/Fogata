import 'dart:async';

import 'package:flutter/semantics.dart';
import 'package:flutter/widgets.dart';

import 'l10n.dart';

/// Tells a screen reader [text] when nothing focused would say it: a message come into
/// the chat on screen, a recording started.
void announce(String text) {
  final view = WidgetsBinding.instance.platformDispatcher.implicitView;
  if (view == null) return;
  // Every language the app has is written left to right.
  unawaited(SemanticsService.sendAnnouncement(view, text, TextDirection.ltr));
}

/// Most messages come in at once that are read out one by one; more are told as how
/// many, so a burst does not keep the screen reader talking.
const toldOneByOne = 3;

/// What a screen reader is told of [messages] come into the chat on screen at once, each
/// as who wrote it and its words: each one while they are few, else how many, and from
/// whom when all are from one person.
List<String> toldOf(List<(String, String)> messages) {
  if (messages.length <= toldOneByOne) {
    return [
      for (final (name, words) in messages)
        l10n.commonNewMessageFrom(name, words),
    ];
  }
  final names = {for (final (name, _) in messages) name};
  return [
    names.length == 1
        ? l10n.commonNewMessagesFrom(messages.length, names.single)
        : l10n.commonNewMessages(messages.length),
  ];
}
