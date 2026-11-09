//! This device's profile and its friends: the profile, banner and bio it shows, its friends
//! and requests as the screens list them, links and codes to make friends with, and the
//! animations, banners and mutual friends of each.

use anyhow::{Result, bail};
use fogata_crypto::DevicePublicKey;
use fogata_friends::{
    Animated, Avatar, Banner, CODE_LIFETIME_MS, FriendCode, FriendLink, LINK_PREFIX, Profile,
};

use crate::{Core, FriendView, Invitation, Presence, RequestView, now_ms, person, person_of};

impl Core {
    pub async fn profile(&self) -> Result<Option<Profile>> {
        Ok(self.inner.node.friends().await?.map(|b| b.profile))
    }

    pub async fn set_profile(&self, name: Option<String>, avatar: Option<Avatar>) -> Result<()> {
        Ok(self.inner.node.set_profile(name, avatar).await?)
    }

    /// Changes the banner friends see on this device's card: a colour (0xAARRGGBB) and a
    /// small PNG or JPEG over it. Nothing happens if it is the one it had.
    pub async fn set_banner(&self, color: u32, picture: Option<Vec<u8>>) -> Result<()> {
        Ok(self.inner.node.set_banner(color, picture).await?)
    }

    /// The bio friends see on this device's card; none until the person writes one.
    pub async fn bio(&self) -> Result<Option<String>> {
        Ok(self
            .inner
            .node
            .friends()
            .await?
            .and_then(|b| b.bio)
            .map(|b| b.text))
    }

    /// Changes the bio friends see on this device's card: at most 190 characters on 4
    /// lines, tidied as names are (see [`fogata_friends::clean_bio`]). Nothing happens if
    /// it is the one it had.
    pub async fn set_bio(&self, text: String) -> Result<()> {
        Ok(self.inner.node.set_bio(text).await?)
    }

    pub async fn friends(&self) -> Result<Vec<FriendView>> {
        let Some(book) = self.inner.node.friends().await? else {
            return Ok(Vec::new());
        };
        let logs = self.inner.node.groups().await?;
        let state = self.inner.lock();
        let mut out: Vec<FriendView> = book
            .friends
            .iter()
            .map(|(device, friend)| {
                let path = state
                    .friends_online
                    .get(device)
                    .filter(|_| !state.hidden.contains(device))
                    .cloned();
                let presence = match (state.friend_rooms.get(device), &path) {
                    (Some(group), Some(_)) => Presence::InRoom {
                        group: group.and_then(|g| {
                            logs.iter()
                                .find(|l| l.group_id == g)
                                .map(|l| (g, l.state.name.clone()))
                        }),
                    },
                    (None, Some(_)) if state.shows_away(device, false) => Presence::Away,
                    (None, Some(_)) => Presence::Online,
                    (_, None) => Presence::Offline {
                        last_seen_ms: state
                            .meta
                            .last_seen
                            .get(&hex::encode(device.as_bytes()))
                            .copied(),
                    },
                };
                FriendView {
                    person: person_of(&friend.profile),
                    fingerprint: device.fingerprint_grouped(),
                    avatar: friend.profile.avatar.clone(),
                    presence,
                    path,
                    delivered: friend.delivered,
                    bio: friend.bio.as_ref().map(|b| b.text.clone()),
                }
            })
            .collect();
        out.sort_by(|a, b| {
            a.person
                .name
                .to_lowercase()
                .cmp(&b.person.name.to_lowercase())
        });
        Ok(out)
    }

    /// Friend requests to answer, and requests this device sent.
    pub async fn requests(&self) -> Result<(Vec<RequestView>, Vec<RequestView>)> {
        let Some(book) = self.inner.node.friends().await? else {
            return Ok((Vec::new(), Vec::new()));
        };
        let incoming = book
            .incoming
            .iter()
            .map(|(device, r)| RequestView {
                person: person_of(&r.profile),
                fingerprint: device.fingerprint_grouped(),
                since_ms: r.since_ms,
            })
            .collect();
        let outgoing = book
            .outgoing
            .iter()
            .map(|(device, r)| RequestView {
                person: person(*device, &r.name),
                fingerprint: device.fingerprint_grouped(),
                since_ms: r.since_ms,
            })
            .collect();
        Ok((incoming, outgoing))
    }

    /// A friend link (also for a QR code) for one person, valid for ten minutes.
    pub async fn friend_link(&self) -> Result<Invitation> {
        let link = self.inner.node.create_friend_link().await?;
        Ok(Invitation {
            text: link.encode(),
            expires_ms: link.expires_ms,
        })
    }

    /// A short friend code for one person, valid for ten minutes.
    pub async fn friend_code(&self) -> Result<Invitation> {
        let code = self.inner.node.create_friend_code().await?;
        Ok(Invitation {
            text: code.to_string(),
            expires_ms: now_ms() + CODE_LIFETIME_MS,
        })
    }

    /// Sends a friend request with a link or code from the other device.
    pub async fn add_friend(&self, link_or_code: &str) -> Result<()> {
        let text = link_or_code.trim();
        if text.starts_with(LINK_PREFIX) {
            let link = FriendLink::decode(text)?;
            if link.is_surely_expired(now_ms()) {
                bail!("this link has expired; ask for a new one");
            }
            self.inner.node.use_friend_link(link).await?;
        } else if text.starts_with("fogata:") {
            bail!("this is a group invite; join the group with it instead");
        } else {
            self.inner
                .node
                .use_friend_code(FriendCode::parse(text)?)
                .await?;
        }
        Ok(())
    }

    /// Sends a friend request to `device`, a member of a group this device is in, over the
    /// connection where it proved to be that member: no link or code needed. It is listed by
    /// the name of the member's profile, or by `shown` (the name its group shows) until one
    /// came. When that member asked first, its request is accepted instead. Whether it got
    /// there comes as an event, as for a link.
    pub async fn ask_member(&self, device: DevicePublicKey, shown: &str) -> Result<()> {
        let name = self
            .inner
            .lock()
            .profiles
            .get(&device)
            .map(|p| p.name.clone())
            .unwrap_or_else(|| shown.to_string());
        Ok(self.inner.node.ask_member_friend(device, name).await?)
    }

    pub async fn answer_request(&self, device: DevicePublicKey, accept: bool) -> Result<()> {
        Ok(self.inner.node.answer_friend(device, accept).await?)
    }

    /// Forgets a friend (this device gets a new friend secret, and the friend forgets this
    /// device too as soon as it hears it) or cancels a request. A call with the friend ends
    /// first, while the friend still takes the hang-up.
    pub async fn remove_friend(&self, device: DevicePublicKey) -> Result<()> {
        self.end_calls_with(device).await;
        Ok(self.inner.node.remove_friend(device).await?)
    }

    /// Makes `picture` (a GIF or WebP, or none) move in place of this device's picture or
    /// banner, whose still picture must be the animation's first frame: set the profile or
    /// banner first. Friends get both, members the picture's.
    pub async fn set_animation(&self, of: Animated, picture: Option<Vec<u8>>) -> Result<()> {
        Ok(self
            .inner
            .node
            .set_animation(of, picture.unwrap_or_default())
            .await?)
    }

    /// The animation of a friend's or member's picture or banner, with the still picture it
    /// moves in place of: only while that is the device's picture or banner now.
    pub async fn animation(
        &self,
        device: DevicePublicKey,
        of: Animated,
    ) -> Result<Option<(Vec<u8>, Vec<u8>)>> {
        let (animation, member) = {
            let state = self.inner.lock();
            let Some(animation) = state.animations.get(&(device, of)).cloned() else {
                return Ok(None);
            };
            let member = match &state.profiles.get(&device).map(|p| &p.avatar) {
                Some(Avatar::Photo(photo)) => Some(photo.clone()),
                _ => None,
            };
            (animation, member)
        };
        let friend = self
            .inner
            .node
            .friends()
            .await?
            .and_then(|mut book| book.friends.remove(&device));
        let stills = match of {
            Animated::Photo => {
                let photo = friend.and_then(|f| match f.profile.avatar {
                    Avatar::Photo(photo) => Some(photo),
                    Avatar::Preset(_) => None,
                });
                vec![photo, member]
            }
            Animated::Banner => vec![friend.and_then(|f| f.banner).and_then(|b| b.picture)],
        };
        Ok(stills
            .into_iter()
            .flatten()
            .find(|still| animation.animates(still))
            .map(|still| (still, animation.picture)))
    }

    /// A friend's banner, once it sent one.
    pub async fn friend_banner(&self, device: DevicePublicKey) -> Result<Option<Banner>> {
        Ok(self
            .inner
            .node
            .friends()
            .await?
            .and_then(|mut book| book.friends.remove(&device))
            .and_then(|friend| friend.banner))
    }

    /// The friends this device has in common with a friend online now, asked of that friend
    /// without either showing its list.
    pub async fn mutual_friends(&self, device: DevicePublicKey) -> Result<Vec<DevicePublicKey>> {
        Ok(self.inner.node.mutual_friends(device).await?)
    }
}
