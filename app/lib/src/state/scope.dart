import 'package:flutter/widgets.dart';

import '../app_state.dart';
import '../direct_state.dart';

/// Gives the state to the widgets below. [of] makes a widget build again on any change of
/// it; [watch] only on changes of one part, so a widget that shows a few parts builds again
/// only when one of those changes.
class AppScope extends StatelessWidget {
  const AppScope({super.key, required this.state, required this.child});
  final AppState state;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    var scoped = child;
    scoped = _Scope<Navigation>(notifier: state.nav, child: scoped);
    scoped = _Scope<Contacts>(notifier: state.contacts, child: scoped);
    scoped = _Scope<RoomState>(notifier: state.roomState, child: scoped);
    scoped = _Scope<LiveCall>(notifier: state.callState.live, child: scoped);
    scoped = _Scope<CallState>(notifier: state.callState, child: scoped);
    scoped = _Scope<Notices>(notifier: state.notices, child: scoped);
    scoped = _Scope<ToastBoard>(notifier: state.toasts, child: scoped);
    scoped = _Scope<Prefs>(notifier: state.prefs, child: scoped);
    scoped = _Scope<DirectChats>(notifier: state.direct, child: scoped);
    return _Scope<AppState>(notifier: state, child: scoped);
  }

  /// The state, building [context] again whenever any part of it changes, except the
  /// toasts, which only the toasts show.
  static AppState of(BuildContext context) {
    watch<DirectChats>(context);
    watch<Prefs>(context);
    watch<Notices>(context);
    watch<CallState>(context);
    watch<LiveCall>(context);
    watch<RoomState>(context);
    watch<Contacts>(context);
    watch<Navigation>(context);
    return watch<AppState>(context);
  }

  /// The state, building [context] again only when one of [parts] changes: for a widget
  /// that shows those parts of the state and nothing else of it. [AppState] among them
  /// stands for what is told to the whole state.
  static AppState only(BuildContext context, Set<Type> parts) {
    for (final part in parts) {
      switch (part) {
        case const (AppState):
          watch<AppState>(context);
        case const (DirectChats):
          watch<DirectChats>(context);
        case const (Prefs):
          watch<Prefs>(context);
        case const (ToastBoard):
          watch<ToastBoard>(context);
        case const (Notices):
          watch<Notices>(context);
        case const (CallState):
          watch<CallState>(context);
        case const (LiveCall):
          watch<LiveCall>(context);
        case const (RoomState):
          watch<RoomState>(context);
        case const (Contacts):
          watch<Contacts>(context);
        case const (Navigation):
          watch<Navigation>(context);
        default:
          throw ArgumentError.value(part, 'parts', 'not a part of the state');
      }
    }
    return read(context);
  }

  /// The state, without building [context] again when it changes: for what is done
  /// rather than shown.
  static AppState read(BuildContext context) =>
      context.getInheritedWidgetOfExactType<_Scope<AppState>>()!.notifier!;

  /// One part of the state ([Piece]), building [context] again only when that part
  /// changes; `watch<AppState>` hears only changes told to the whole state.
  static T watch<T extends ChangeNotifier>(BuildContext context) =>
      context.dependOnInheritedWidgetOfExactType<_Scope<T>>()!.notifier!;
}

class _Scope<T extends ChangeNotifier> extends InheritedNotifier<T> {
  const _Scope({required T super.notifier, required super.child});
}
