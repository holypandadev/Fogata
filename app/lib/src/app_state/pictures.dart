part of '../app_state.dart';

/// The pictures this person shows others and sees of them: their own banner and
/// animations given to the core, and the animations, banners and friends in common
/// asked of others.
extension _Pictures on AppState {
  void _forgetOwnBanner() {
    final id = contacts.profile?.person.id;
    if (id != null) movingBook.forget(id);
  }

  /// Gives the core the banner's animation, after the banner it moves in place of.
  Future<void> _moveBanner() async {
    if (!started) return;
    try {
      await api.setAnimation(
        of: UiAnimated.banner,
        picture: prefs.movingBanner,
      );
    } catch (_) {}
  }

  /// Gives the core this person's animations at start: they go to friends, and the
  /// picture's to members, as each connects.
  Future<void> _shareMoving() async {
    if (prefs.movingBanner != null) await _moveBanner();
    if (prefs.movingPhoto != null &&
        contacts.profile?.person.avatar.photo != null) {
      try {
        await api.setAnimation(
          of: UiAnimated.photo,
          picture: prefs.movingPhoto,
        );
      } catch (_) {}
    }
  }

  /// Asks the core for a friend's or member's animation; this person's own are known here.
  Future<void> _askMoving(String id, {required bool banner}) async {
    await null;
    if (id == contacts.profile?.person.id) {
      final still = banner
          ? prefs.bannerPicture
          : contacts.profile?.person.avatar.photo;
      final picture = banner ? prefs.movingBanner : prefs.movingPhoto;
      movingBook.put(
        id,
        still == null || picture == null
            ? null
            : (still: still, picture: picture),
        banner: banner,
      );
      return;
    }
    try {
      final got = await api.animation(
        deviceId: id,
        of: banner ? UiAnimated.banner : UiAnimated.photo,
      );
      movingBook.put(
        id,
        got == null ? null : (still: got.still, picture: got.picture),
        banner: banner,
      );
    } catch (_) {}
  }

  /// Gives the banner to the core, which sends it to friends if it changed.
  Future<void> _shareBanner() async {
    if (!started) return;
    final picture = prefs.bannerPicture;
    try {
      await api.setBanner(
        color: prefs.bannerColor.toARGB32(),
        picture: picture == null ? null : await friendBannerPng(picture),
      );
    } catch (_) {}
  }

  /// The friend's banner, and the friends in common if the friend is online: asked again
  /// at most once a minute.
  Future<void> _loadFriendCard(String id) async {
    try {
      contacts.friendBanners[id] = await api.friendBanner(deviceId: id);
      contacts.changed();
    } catch (_) {}
    final online = contacts.friends.any(
      (f) => f.person.id == id && f.presence != UiPresence.offline,
    );
    final now = DateTime.now().millisecondsSinceEpoch;
    if (!online || now - (contacts.mutualAskedMs[id] ?? 0) < 60000) return;
    contacts.mutualAskedMs[id] = now;
    try {
      contacts.mutualFriends[id] = await api.mutualFriends(deviceId: id);
      contacts.changed();
    } catch (_) {
      // Asked again the next time the card opens.
      contacts.mutualAskedMs.remove(id);
    }
  }
}
