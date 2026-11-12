Part of [Fogata architecture](../architecture.md).

## 5d. The desktop application (app/, fogata-ffi)

A Flutter application on top of the core. `crates/fogata-ffi` is the bridge
(flutter_rust_bridge 2.13): it starts the core once on its own Tokio runtime and gives the
screens plain values (ids as hex strings, times in milliseconds) and one stream of events.
That runtime has one worker per core, at most four; the bridge's calls run on it too, the
blocking ones on its pool for blocking work (`crates/fogata-ffi/src/bridge.rs`, in place of
the bridge's default handler, which started a runtime and a pool of threads of its own, each
as large as the computer has cores).
`flutter_rust_bridge_codegen generate` (run in `app/`) writes `app/lib/src/rust/` and
`crates/fogata-ffi/src/frb_generated.rs` from `crates/fogata-ffi/src/api/`.

The window's state (`app/lib/src/app_state.dart`) is one `AppState` that does what the
person asks and follows the core's events, holding its data in parts (`app/lib/src/state/`),
each a `ChangeNotifier` told only of its own changes: `Prefs` (what `app.json` keeps, the
banner's picture and the animations), `ToastBoard`, `Notices` (the bell, missed calls,
declined invitations; it reads friends, requests and invitations through `NoticeSources`),
`CallState` (the room's `Call` and the direct calls) with `LiveCall` (what the call says as
it goes: someone connects, mutes, shares), `RoomState` (the room shown, what the window
remembers of rooms, previews, files waiting to go), `Contacts` (the profile, groups,
members, friends, requests, how each member is reached) and `Navigation` (the page, the
group selected, what floats over the window), besides `DirectChats`. Who is talking
(`Call.talking`) and who is typing (`AppState.typingChanged`) are told apart again.
`AppScope` gives the state to the widgets: `AppScope.of` builds a widget again on any
change (all parts but the toasts), `AppScope.only(context, {...})` and `AppScope.watch<T>`
only on changes of the parts named, and `AppScope.read` never. The room page, the side
column, the rail, the settings, the direct chats and the toasts depend only on the parts
they show: a line in a room's chat no longer builds the side column or the settings, a
toast builds only the toasts, a direct chat's change builds nothing of a room, and what the
call says as it goes builds the room's people and controls (and the column's rows of
people) but not its chat (`test/rebuilds_test.dart` counts it). `AppState.notifyListeners`
still tells every part, for the changes that touch many of them (the groups or friends
read again, a call starting or ending).

The screens follow the design canvas, in dark and light, laid out like a chat application
people already know: a rail on the left with home on top and every group as an icon (with a
badge while its room is live), a column beside it for what is selected (for home: the
overview, friends and settings; for a group: its members and invite, then its voice rooms
with who is in them), the page in the centre and, in a room, the chat and files beside it,
each in a tab of their own. The person's own pill sits at the bottom left, under both the rail and the
column, with the microphone, deafen and the settings (in the rail instead when the window
leaves no room for the column). Beside the microphone and deafen there, and beside the
microphone, deafen and camera in a room's controls, a small arrow opens that device's panel
attached to the button (over it, the arrow pointing up meanwhile): the devices of its kind
to choose from, the system's default first, noise suppression under the microphones, and the
way to Voice & video in the settings. A choice applies at once, also in a call, and is kept.
The list follows devices plugged in or out while it is open (read again every 4 s, as in the
settings). The names the system's default also goes by in WebRTC's lists ("default: " and the
device's name on Linux, "default" and "communications" on Windows, "default" on macOS) and
nameless entries (macOS lists one for the default input of a computer without any) are left
out there and in the settings, since "System default" is that; one chosen before counts as
"System default".
Clicking the pill opens their panel: the top in a
colour or picture chosen in the settings (friends see it on the card too), picture with
status, name, bio, edit profile (it opens My profile), Online / Offline (appear offline; away comes by itself, 3.9) and a button that copies a new friend
link. The pages: welcome (what Fogata is for, then make the identity), home
(invitations from friends, live rooms first, then the rest), new group (an icon, or a
picture from the computer, set as the group's picture record right after `Create`), friends (QR, link or
code, requests, presence, a Call button for each friend online, missed calls, and a
menu on each friend's "···" to add them to a group or remove them, which asks first), a group's members (in their order, how each is reached; managers
invite with a link; each moves up or down, times out and removes those below; anyone leaves,
the first too),
settings and a group's room (people, chat with files and who came and went,
files with progress, "connecting", "only you" before closing a room, and "could not
connect"). A direct call (3.11) shows as that room with its own title, "Calling …" with
rings while it rings, a card when a friend rings (a small one in the corner while in a
room, saying that answering leaves it), why a call did not go through, and missed calls in
a corner note (it goes by itself after 8 seconds, or with a click, like a toast) and in
Friends and the bell. A friend in the same voice room as this device is not rung: the
Call buttons say "You're both in" that room instead. The interface speaks English, Spanish,
Brazilian Portuguese, French, German, Italian, Russian, Japanese and Simplified Chinese, the
system's language at first (`docs/translating.md`). Links, QR codes, short codes and invitations are
blurred until an eye button shows them for ten seconds, and a copy button copies them
without showing, so nothing leaks from a window on a shared screen.

The settings float over the window: a column lists the parts under
two headings, User (My profile: picture, name, bio, the card's background and this
computer's fingerprint) and App (Appearance: theme; Language & time: the language, each
named in itself, applied at once and kept as `uiLanguage` in `app.json` once chosen (until
then the system's), and the time format; Voice & video: the
microphone with noise suppression and the speaker, then the camera in a box of its own with
Test video, a preview only this device shows: it opens the camera, as a call does, only
when pressed, and closes it when the part or the settings close, the window is hidden or
minimized, or a call opens the camera; and Mirror my camera (on by default, kept as
`mirrorCamera`), which draws this device's own camera as in a mirror wherever it shows here,
while the others always get its picture as it is. Test microphone opens the chosen
microphone as a call does and shows its level: WebRTC records, and tells the level, only
while a connection sends, so the track goes from one connection to another in the same
process and the sending one's statistics say how loud it is; nothing leaves the computer.
Hear myself (off until turned on) plays it through the chosen speaker. The test closes when
the part or the settings close, the window is hidden, or a call starts, which stops it
before anything else; alone in a call, the own tile's level comes from such a loop too;
Sounds; Behaviour: opening Fogata when the computer starts, off by default and read from
the system rather than kept by Fogata (an entry in `$XDG_CONFIG_HOME/autostart`, by
default `~/.config/autostart`, on Linux; inside a Flatpak, the Background portal's
`RequestBackground` with `autostart`, through the runner's `fogata/desktop` channel, with
the sandbox's own entry remembering its answer; the application as a login item through
`SMAppService` on macOS 13 and later; a `Fogata` value under the user's
`Software\Microsoft\Windows\CurrentVersion\Run` key on Windows), and keeping running in
the tray, where there is one; Network: whether this
device relays for others, and the network check; Storage & downloads: the downloads folder,
what direct messages take and the data folder), and the rest shows the chosen part. Where the
two do not fit side by side (narrower than 680 points), the list and a part take turns, with
a button back to the list. Closing the settings (Esc, a click beside them, the close button,
or going to another place) while My profile holds changes not saved asks first: save them,
discard them or keep editing. The settings and the other pages that scroll as a whole glide
with each step of a mouse wheel (200 ms, the steps adding up while the wheel turns), as
browsers do, rather than jump; a touchpad's small steps move them as they come. A friend's card (Friends, and the right side of a direct chat)
shows the friend's banner, picture, name, code, status in words and bio, then Call and
Message side by side (one under the other when their words would not fit so), and the
friends in common.

The window has six themes, each one set of the same colour tokens (`Fg` in theme.dart): Dark
(the default), Midnight (true black), Dim (softer greys), Ember (warm greys), Contrast (black
and white with stronger lines and colours) and Light, plus System, which follows the computer
between Dark and Light. Every theme's text colours read at 4.5:1 or more on its surfaces
(checked by test/theme_test.dart). The choice is kept as `theme` in `app.json`; versions
from before the extra themes read any name but `light` and `system` as Dark.

Text follows the system's text size up to 1.5 times (`maxTextScale` in shell.dart); larger
settings stop there, the most the window at its smallest (960 by 640) holds. Rows that hold
text keep their height as a minimum and grow with it rather than cut it.

On a screen whose work area is under 960 by 640 (a scaled laptop screen, a Mac at "Larger
Text") the window's smallest size is that area (`mainWindowSizes` in window.dart), and the
pages fit it too: test/languages_fit_test.dart tries every page at 1024 by 571 (text at its
size and 1.5 times), 900 by 600 and 800 by 560 besides 960 by 640. What does not fit scrolls;
the tile of a room this device is alone in grows smaller; a card's two buttons go one over
the other, and a dialog's buttons onto two lines, when their words would not fit; under 860
points wide only the rail shows beside the page, without the column and the person's pill.

Every button can be reached with Tab (with a blue ring) and used with Enter or Space. Tab
goes through the title bar, the rail, the column and the page in turn; while the settings, a
group's settings, the person's panel, a friend's card, the bell, a device panel or a menu
float over the page, Tab stays inside them and Esc closes them (a device panel or a menu
gives the focus back to the button that opened it). The panel, the card and the bell open
beside the button that opened them and, like device panels and menus, stay beside it when
the window changes size (placed again from where the button is then); the bell's
(380 points wide, 340 in a window under 1000) hangs under the bell with their right edges
lined up, inside the window, its notices scrolling when it is short. Device panels and
menus open over or under their button, wherever there is more room, and always inside the
window: moved in from its edges and, when taller than the room, scrolling.

Ctrl K (⌘K on macOS), or the round button in the title bar (the shortcut in its tooltip),
opens the quick switcher from any page: in the middle of the window (as tall as it opens:
typing shortens its list from the bottom and the field stays put), a field and the direct
chats, friends, voice rooms and groups whose names match it (a voice room also by its group's
name). With nothing typed it lists the places shown lately in this session, then the chats
newest first, the voice rooms with people in them, the groups and the other friends; typed,
names that start with the text come first, then those with a word that does. Arrows choose,
Enter goes there (a voice room asks the same questions as the side column before leaving a
call or closing a room) and Esc closes it. Friends filters its list with its own field.

Both message boxes (a direct chat and a room's chat) have a smiley button that opens the
emoji picker above it: a search by name, the skin tone, the groups as icons to jump to, the
recently used first and the emoji in a grid, with the name of the one under the pointer or
chosen. The arrows choose from the search field, Enter or a click puts the emoji at the
box's cursor and closes it (with Shift it stays open), and Esc closes it. It stays inside
the chat's column (a room's chat beside the call takes 7 emoji to a line instead of 8 to
fit) and the window, and it closes, as the ":name" suggestions do, when the chat hides
(by hand, or a window made smaller) or goes offstage. Typing ":name" in
a box offers the matching emoji above it (Enter or Tab takes one, Esc leaves the text), and a
whole ":name:" that is an emoji's name or a common short name ("joy", "+1") becomes it. The
names come from Unicode's emoji-test.txt (`app/tool/emoji_table.py` writes
`lib/src/emoji_table.dart`), up to Emoji 13.0 so every system's emoji font has them; they
are drawn with the system's colour font (Apple Color Emoji, Segoe UI Emoji, or Noto Color
Emoji on Linux, where the picker says when fontconfig has no colour emoji font). Country
flags are left out on Windows, whose font draws them as two letters. The interface is set in
Inter (SF Pro on macOS), headings (page titles, the group's name over its column, the voice
rooms' names) included, in bold (`theme.dart`'s `hx`); only the wordmark, "Fogata", is in the
app's own face, Fogata Ember (`wordmark`, `assets/fonts/FogataEmber-Heavy.ttf`). The recently used
emoji and the skin tone are kept in `app.json`. The search also matches the names and
keywords of the language in use, then English's (`assets/emoji/<code>.tsv`, from Unicode
CLDR, read the first time the picker or `:name` needs them, away from the window's thread;
see docs/translating.md).

A group a friend added this device to arrives as an invitation on the home page: nothing
of it runs, is announced or is sent until the person joins it. Declining forgets its log and
is final: friends in the group offer its log again each time they meet this device, and
`groups.json` keeps the decline (the group id and the entries that had added this device;
older versions keep such a record as one they cannot read), so those offers are ignored.
The device also signs a `Leave` on the log it was offered and hands it to the members that
are its friends, at once to those online and later to whoever offers the group again; once
they take it, the group no longer counts the device, stops offering it the group, and an
manager can add it again. Only such a new addition (an `Add` or `Join` of this device the
decline did not see) brings the invitation back. Older versions take the `Leave` like any
entry a member hands them.
The `Leave` goes in `FriendRequest::Group` with `declined`, the hash of the
entry that made the invitation (the latest `Add` of the device, or its `Join`). The member
that made it (the author of the `Add`, or for a `Join` the manager who wrote the invite)
tells its person "<name> declined the invitation
to <group>" in the bell and a toast (`InvitationDeclined`), once per invitation while it
runs; the other members only take the `Leave`. It takes the request also from a device no
longer a member, when another member brought the `Leave` first. Older versions ignore the
field, and older decliners send none, so their declines still show nothing. An inviter that
was away when the person declined hears it when it next offers the group to them, unless
another member brought it the `Leave` before.
At most 8 invitations from one friend and 32 in all wait at once (further ones are refused
until the person answers some), an offered log holds at most 4096 entries and 4 MiB, and
the bell announces at most three invitations per friend a minute; the others still show in
the list.
Group and chat ids are random, so the same id twice was chosen on purpose: a friend's
invitation or chat under the id of a group being joined with an invite is refused, a chat
that already has that id gives it up (its messages stay; the next one makes a new chat
group), and the log the manager sends replaces an invitation kept under that id. A decline
holds only for the group declined, told by its first entry: a friend that offers a made-up
log under a real group's id first does not keep the real group out once the person declines
the made-up one.

Direct messages (3.13) list under Home and Friends in the column, newest first, each with
the friend's picture and presence, the last line and the messages unread; the rail's home
badge counts them too, the bell shows a chat with unread messages, and someone else's
message makes the room's soft pop unless that chat is being read. Friends and a friend's
card open a chat with Message. A chat's page has the friend's name and status, the
disappearing timer as a pill that opens the four choices, and the call button (off while the
friend is offline); the messages by day and by person (one name for the messages a person
writes within five minutes), the timer's changes as lines, pictures, videos (their first
picture, darker until fetched) and files with what can be done with each, and under this
device's last message whether it was delivered or waits for the friend; the box to write
(with + for files and the emoji picker); and on the right the friend's card with what the chat takes here and
"Free up space…", which opens the settings at Storage & downloads. Once the two are no longer friends
the box gives way to a line and Delete chat. Settings › Storage & downloads shows what direct messages
take, the room for their pictures, videos and files (a smaller one than what they take asks
first), what comes by itself, the timer of new chats, and each chat's Clear files.

The room shows a tile per person (a green ring and bars while they talk, a crossed
microphone when muted, their camera when it is on) and a wider tile per shared screen. The
controls turn the microphone, camera and screen on and off, pick a screen or window to
share, open the device choices, hide or show the chat and leave. Files picked with the
chat's clip or dropped anywhere on the room (`desktop_drop` 0.8.0, our copy in
`app/third_party/desktop_drop`: later versions depend on the MPL `dbus` package) wait above
the message box, each with its size and the folder it is in, a small picture for images and
a button to take it out; only files of this computer go: on Linux a drop is taken only as a
list of files (`text/uri-list`, its `file:` entries), never as text that reads as a path, and
a path that is not absolute, a device or a pipe is left out (the core refuses to share
anything but a regular file, too). Any other target a browser or a file manager offers
(`text/x-moz-url`, `_NETSCAPE_URL`, `x-special/gnome-copied-files`, plain text) adds nothing.
A web page can start a drag whose list names any file of the computer, whatever the browser,
so a dropped file that is hidden (a name that starts with a dot), in a private place (a macOS
`Library` outside iCloud Drive and the File Provider folders in `CloudStorage`, Windows
`AppData`, the system folders of any drive or share, Linux `/run` outside the portal's
documents and `/media`) or a link that leads to one is never added by dragging: the rest is,
and a notice says how to share such a file on purpose, picked with the clip, which no page
can do. Names are read as the system reads them: `..` worked out, without their case on macOS
and Windows (the long s and the Kelvin sign as s and k), Windows' trailing dots and spaces
and its short names (`APPDAT~1`), and where each really is, links followed. The user's
Library is looked for under the account's own folder as well as under `HOME`, which macOS's
App Sandbox points at the application's container. Inside a Flatpak, files dropped from
outside the sandbox arrive as the document portal's paths (`/run/user/<id>/doc/...`), and
it is the portal that decides what the application may read: only what the person dragged
is exported. A file waiting or being sent is not added again; files that fail wait again without
changing the tab, and a send stops when the room is left. Files picked with the file picker do not ask. Send (or Enter) shares them with the message
typed, if any, which goes first, and Esc takes them all out. Folders are left out, and
said so. A file that follows its sender's message by a few seconds shows under it. A right
click (or a long press) on a person, on their tile or in the room's list, opens their
voice volume. On the groups page a
live room's card has the same microphone and camera buttons for entering.

A watched screen, or a camera enlarged with a click, fills the room in its own shape with
the others in a strip under it. The chat and files beside a room or call are hidden and
shown again by hand with the chat button among the controls (a direct call's Chat or
Files in the side column shows them too); hidden, they keep what was typed. On a wide window the choice is
kept in the app's settings (`roomChat` in `app.json`) for every room and call, and
nothing hides them by itself. A small window (a room page under 832 points, where the
chat beside the call would leave the controls less than their whole row: a window under
about 1144 points, 24 more before it counts as wide again) starts every call with them
hidden; shown there by hand, or with Chat or Files, they stay shown for that call however
the window changes, until hidden by hand. Shown on a small window they are shown on wide
ones too, and hidden on a wide one they are hidden on small ones too. While another page is
shown, or the settings cover the room, someone else's screen or camera floats over the
window instead (`screens/pip.dart`): in the bottom right corner above the corner's cards,
with whose it is and, under the pointer, a button that stops watching. It can be dragged
anywhere under the header and stays inside the window as the window changes size (pushed
back into the corner, it returns to its place above the cards); a click goes back to the
room or call, where it fills the room again. It goes when the screen or camera ends, the
person leaves, or the room or call does. It draws the renderer the room draws, so nothing
more is received. Toasts keep clear of the corner: between the side column and the
corner's cards, or above them when that space is narrow. They show over everything else,
so a note said from inside the settings, a group's settings or a dialog is seen there.
Clicks go through them, except on one about a file ("Saved to …"): it stays 10 s, with
Show in folder and Copy path. The microphone
stays as it was left in the last call; the camera starts off every time. macOS asks for the
microphone and the camera the first time they are used (`Info.plist` and the entitlements
explain why).

Sounds for the room this device is in: a fourth going up when this device
or someone else comes in, the same fourth going down when someone leaves, two soft low notes
a third apart when someone else writes (at most one per 0.8 s), a quick step down when this device mutes
and up when it unmutes, three notes up when anyone starts sharing a camera, a screen or a
window and down when they stop, and two light high notes when someone starts or stops
watching a screen. Around the rooms: a two-note chime when the bell gets something new (a
missed call, a friend request, someone asking to join, a group a friend added this device
to, this identity open on another computer too; at most one every two seconds, nothing for
what was waiting at start), a fall to the low tonic when a direct call ends or does not go
through, and the rings of a direct call (a short song on a soft felt piano, one bar of
3.2 s, while a friend calls, a slow soft rise up the chord while this device calls), which
sound even with the sounds off. The player itself starts a ring again every 3.2 s, each bar
over the tail of the one before and on the beat however long the first took to make, until
told to stop: answering, declining or hanging up stops it at once, before the core is done,
faded out over 30 ms while other sounds play on. The rings take about a second to make, so
they are made ahead on a thread of their own when the core starts (at 48 kHz, and at the
output's rate once it opens at another). Nothing sounds for
the room's past arriving in the first three seconds after entering, nor for files. The
sounds are synthesized in
`crates/fogata-ffi/src/sounds.rs` as one family (a soft mallet on a glassy bar, and a warmer
FM mallet for the start of sharing, mostly with a light stereo chorus, a short room and a
gentle EQ, in A major pentatonic, each brought to about the same loudness and kept under
-1 dBFS; each also has other variants to compare by ear, and one table there picks the one
each plays). They are made in stereo, an output with one channel getting both sides mixed, and
played with cpal on the output chosen in Settings (the speaker the calls use; the system's
default while it is not connected); Settings turns them off (`app.json`). One thread
plays them, mixing sounds that overlap and keeping the output open for fifteen seconds after
the last one, so sounds close together do not each open the device; an output that stopped
(its device went away) is opened again for the next sound, and each different failure to
play is a warning in the log the first time it happens. On Linux they go through ALSA to
PipeWire, which needs ALSA's plugin for the sound server on the computer (`pipewire-alsa`,
or PulseAudio's ALSA plugin under PulseAudio): without it ALSA's default output is the sound
card itself, and the sounds fail while a call holds it. Before the first sound the player
looks for that plugin among ALSA's devices and, missing, says so in the log, naming the
package. The runner names that
stream "Fogata" (`PIPEWIRE_ALSA`), so the system's volume controls show it, and keep its
volume, as Fogata's, and sends it wherever `PULSE_SINK` sends the rest of the
application's sound. There the chosen speaker is reached through the sound server
instead: flutter_webrtc's output keeper (`fogataSoundsTo`) moves every stream of this
process (the sounds, the chat videos, the call) to the chosen output as it appears. Chat
videos play on the chosen output too: on macOS the player is given it
(`audioOutputDeviceUniqueID`), and on Windows the media engine is given its endpoint
(`IMFMediaEngineAudioEndpointId`; the voice engine lists outputs by endpoint id, or by name
when there is none), when it is connected as the video opens; otherwise they play on the
system's default.

Voice messages in direct chats (3.13) are recorded and played in the Rust library
(`crates/fogata-ffi/src/voice/`, files by `crates/fogata-voice`). The microphone button
beside the message box starts recording at one click; the box becomes the recording bar
(Delete throws the recording away without asking, and so does Esc up to 5 seconds, after
which the line under the bar stops offering it so a key pressed by habit never throws away
minutes of talk; also while the microphone is still
opening: once open it closes again; not while it is being sent, when it is on its way; Send
or Enter sends it), with the
time, the last minute counted down from 14:00, and the voice's bars as they come. One under
half a second is not sent: Send throws it away and a toast says it was too short. At 15
minutes recording stops by itself and waits for Send or Delete; so does a recording whose
chat is left (even while its microphone opens), whose friend goes offline, whose window is
hidden in the tray or minimized (nobody sees the recording bar there), or when a call starts; while it waits, the button in
another chat starts nothing and says to send or delete that one first. It cannot start in a direct
call or a voice room, where the call has the microphone. The friend is not told that
anything is being recorded. Recording uses the microphone chosen in Settings › Voice &
video. On Linux the voice engine names it by its description in the sound server; its side
looks up the server's own name for it (`fogataInputName`, FOGATA.md), which ALSA's plugins
for the sound server open (`pulse:DEVICE=…`, then `pipewire:NODE=…`), then ALSA's default,
then cpal's. On Windows and macOS the voice engine names it by the system's id (endpoint id,
Core Audio uid), and recording takes the cpal input of that id, or of that name, else the
system's default input. When the chosen microphone is not the one that opened (not there, or
not found), the line under the recording bar says so: the default microphone records. On macOS it asks for the
same permission calls use. The sound is made mono at 48 kHz, cleaned of noise with the
calls' suppressor when that is on, and encoded at once: Opus for voice at about 24 kbit/s
(constrained variable rate), 20 ms a packet: a minute takes 130 to 175 kB, 15 minutes
1.9 to 2.6 MB. A microphone that does not open, gives nothing for two seconds, or (on macOS,
where one the system has not allowed records digital silence) gives only silence, shows a
note over the box with a way to Settings › Voice & video. One that stops giving sound part way
(an error, or nothing more for two seconds) stops the recording, which is kept to send, with a
note over the recording bar that says so. What the window asks for every 80 ms (where
the recording stands), and stopping or throwing it away, never wait: sending waits up to
300 ms for the microphone to close, so all it took is in, without holding the recording.
`fogata-voice` writes the Ogg
pages itself and reads a friend's file strictly: every page's checksum, its stream, its
place and flags, packets no longer than 8 kB and of a length their first byte allows, at
most 4 MB and 15 minutes and a second in all, nothing after the last page; anything else
does not play. One message plays at a time, decoded as it plays, at 1×, 1.5× or 2× (the
pill on the message), faster ones stretched in time by WSOLA (20 ms pieces, each taken
within 5 ms of its place where it best continues the one before), so the voice keeps its
pitch. It plays with cpal on the chosen output, as the sounds do, with a buffer of about
40 ms, so pausing and the time shown follow at once; the output closes four seconds after
nothing plays. The output's callback decodes holding the player alone, with room made
beforehand for all it keeps; where the message is and whether it plays are kept apart, so
what the screen reads every 80 ms never waits for the decoding. Speed and place are
forgotten when the chat closes. A message that is deleted or whose timer runs out while it
plays or is paused stops, also when newer messages pushed it out of what the chat has read
(the core is asked about it).

What the window keeps of a room (who came and left, files whose holders left, when each
chat line arrived, which is the order the chat is shown in) lives only in memory, like the
room, and is bounded: the last 1000 notes (with the files they show) and the arrival of the
last 1000 chat lines. Messages, in rooms and direct chats, are drawn without the marks that
set the direction of what follows (embeddings, overrides, isolates: one can make
`photo\u{202E}gnp.exe` read as `photoexe.png`) and with at most 4 marks on one character (a
few are accents, hundreds smear over other lines); a line in a script written right to left
is set apart in its own direction. Only the drawing changes: they are kept and sent as
written. The animations that run for as long as something is shown (a live room's dot, someone
talking) step twenty times a second in their own layer, and the room's page is rebuilt when
its state changes, not every second; so an open room does not keep the window redrawing. The only file the application adds to the data folder is `app.json`
with the chosen theme, whether sounds are on, whether the microphone is on, and the chosen
devices; the core writes it, readable by the owner only like the other files there. It also holds
whether this device relays other members' traffic (`bridge`, on by default; each relayed
circuit is capped at 2 GB), which the core reads when it starts. "Readable by the owner
only" is a mode on Linux and macOS (folders 0700, files 0600); Windows has no such modes,
and the data folder, in the user's profile, and what is in it keep the permissions they
inherit from that profile, which lets in only that user, the system and administrators.
The window's other files (the banner's picture and the animations, the note left for a
restart, problem reports, its own log) are written through the core the same way
(`write_private`: owner only, through a temporary file, a link where it goes replaced
rather than followed), and the data folder and its `window` folder are made private before
the window opens its lock file there. A field of `app.json` that cannot be read (a setting
that is on or off and is anything but true or false included) keeps its default; the file
is then kept as `app.json.bad` and the bell says so once.

The app raises its limit of open files and sockets at start (to the system's maximum, at
most 10240): desktop apps on macOS get 256. The network keeps about 50 open in normal use
(DHT nodes are reached over QUIC and their connections close soon, see 3.2); the higher
limit leaves room for calls, files and bursts of TCP connections.

The window has no system title bar: the application's header is the title bar (dragging
its empty parts moves the window, a double click maximizes it). On Windows and Linux it ends
with Fogata's own minimize, maximize/restore and close buttons; on macOS the content reaches
the top edge and the system's traffic lights sit in the header, centred by an empty unified
toolbar (`macos/Runner/MainFlutterWindow.swift`), with room left for them except in full
screen. On macOS release builds run in the App Sandbox (network and user-chosen files and
Downloads allowed) and with the hardened runtime (no JIT, no debugger, only libraries signed
by the app's own team: `tools/sign-macos.sh` signs every framework, see `docs/releasing.md`);
development and test builds run outside the sandbox so tests can use their own folders. On
Windows the runner's first step limits where libraries are looked for to the application's
folder and System32 (`SetDefaultDllDirectories`), so a library left in the current folder or
on PATH is never loaded. Release builds open the Rust library by its own name only; debug
builds also look for it in the source tree, for tests. Linux keeps GTK's client-side
frame (shadow and resize edges) by giving the window a hidden header bar; Windows hides its
caption through window_manager at start. On Linux under Wayland with a fractional scale
(125 %, 150 %) the window looks slightly blurry: GTK 3, which Flutter's Linux embedder runs
on, only gives it a whole scale, so it is drawn at the next whole scale and the compositor
shrinks it. Whole scales (100 %, 200 %) and X11 are sharp. A known limit of Flutter on
Linux, not something the app can set.

On Linux quitting destroys the window, and with it Flutter's view, while the engine still
runs: by itself the engine shuts down a moment later, when its last reference goes, from
the main loop. Two faults of Flutter's Linux embedder made that quit crash at times. The
view frees the compositor that frames are drawn into when its widgets go, so a frame the
engine's raster thread was presenting at that moment aborted the application (GLib refuses
to clear the compositor's locked mutex) or finished in freed memory. And the engine's
platform task runner can leave a timer behind (one posted from another thread while an
earlier one ran), which ran on the freed task runner at the main loop's next turn. The
runner (`linux/runner/my_application.cc`) keeps the view's widgets and the engine from the
window's destruction: the engine shuts down once the main loop has ended (the runner's
application object goes), its frame finished and its threads stopped, and the widgets go
after it. Windows takes the view from the engine, waiting for its frame, before destroying
it, and macOS shuts the engine down as the application terminates, its window still there.

The bridge library is built with `cargo` from the workspace on every desktop system
(`app/rust_builder/linux/CMakeLists.txt`, `windows/CMakeLists.txt`, and on macOS
`macos/build_rust.sh`, the pod's script phase, which joins one static library per
architecture with `lipo`), with `--locked`: the crates are the ones `Cargo.lock` names and
the Rust the one `rust-toolchain.toml` names, and nothing is fetched or run for the build
itself (cargokit, kept for mobile, ran a Dart tool whose packages came from pub.dev on every
clean build). libopus (opusic-sys) is built with CMake: on Windows the build passes its own
CMake (Visual Studio's, often not on the `PATH`) as `CMAKE`, which the `cmake` crate uses,
with the Ninja beside it as the generator (a Visual Studio project built from inside the
app's build, itself one, fails in CMake's compiler checks with MSB4018); on macOS the script looks for it on the `PATH`, then in Homebrew's and CMake.app's places.
On Windows everything Rust builds (the bridge library, the command-line program) has
Control Flow Guard, as the runner and the plugins have (`/guard:cf`): `.cargo/config.toml`
adds `-C control-flow-guard` for MSVC targets. Cargo runs from the workspace, where it finds that file. On Linux and Windows
the build runs cargo through `app/rust_builder/cargo_build.cmake`, which looks at the build's
environment when cargo starts and leaves out, and names in the build's output, every
variable that would change the code cargo makes or what makes it: `RUSTFLAGS` (which would
replace those flags), `CARGO_TARGET_*`, `CARGO_BUILD_*` (but the number of jobs),
`CARGO_PROFILE_RELEASE_*`, `RUSTUP_TOOLCHAIN`, the compiler and flag variables that reach
libopus and AWS-LC (`CC`, `CFLAGS`, `CFLAGS_<target>`, `TARGET_CFLAGS`...), MSVC's `CL` and
`LINK`, and `CMAKE_*`. `tools/beta/release.py` then refuses a Windows build in which any
`.exe` or `.dll` lacks ASLR, DEP, Control Flow Guard or EH continuation metadata
(`/guard:ehcont`), except what each cannot have: the bridge library has no EH continuation
metadata (no stable rustc option makes it), nor does `libwebrtc.dll`, built with WebRTC's
own toolchain, without C++ exceptions, and `flutter_windows.dll`, Flutter's engine as its
upstream release ships it, has neither. The runner and the plugins have both on x64. CET shadow stack
compatibility is off on purpose: Windows turns shadow stacks on for a process when its
program says it is compatible, Fogata was never tried on a processor that has them, and a
mistake there would end it at start on every such computer, which is worse than going
without that protection. The runner and the plugins are linked with `/CETCOMPAT:NO`, so a
linker whose default changes does not turn it on, and `tools/beta/release.py` refuses a
`fogata.exe` that is compatible. `libwebrtc.dll` comes compatible from its own build, which
changes nothing while the program is not. It is to be turned back on once Fogata has been
tried on such a processor. On Linux the runner is a position-independent executable
(`-fPIE -pie`).

**The splash.** Every start opens a small window first
(`app/lib/src/splash.dart`): 300 by 360, no frame, a dark card (round corners on Linux,
where its corners are see-through when the desktop composites windows; macOS and Windows
round the window themselves) with the animated logo (`assets/splash/`, twice the size for
screens of high density) and what is happening under it: starting, connecting (the core
starts), checking for updates (friends, members and the releases page are asked; the splash
waits up to six seconds for a newer release, less once those asked said there is none),
downloading (a bar, how much, from whom), installing and restarting. A release that is
ready, or whose download moves, is installed there and Fogata starts again as the new
version; a download that stops for 15 s, or that would take more than three minutes at its
pace so far (unless this copy must update), goes on behind the main window. A release that
becomes ready while the main window shows is announced once in a toast, and the title bar's
pill (green, amber when this copy must update) restarts into it. The splash shows at least 0.6 s. The runners make the
window the splash before anything shows (Linux in `my_application.cc`, macOS in
`MainFlutterWindow.swift`, Windows in `setUpWindow`, around the middle of the window made
for the main one), and `becomeMainWindow` (`window.dart`) gives it back its frame, window
buttons, smallest size (960 by 640, or the screen's work area if that is smaller, so the
window's bottom is never kept under the taskbar or the Dock) and 1440 by 900 (or the
screen's work area) in the middle of the screen (`mainWindowSizes`); on Linux through the channel `fogata/desktop` (`mainWindow`), which
hides the window while it changes and shows it again, so that on Wayland, where a window
cannot place itself, the compositor places it as a new one. (GTK sizes a window that
cannot be resized by what its contents ask, so the Linux splash is resizable, only with no
frame to do it from.)

**Updates while running.** When a newer release is ready the title bar shows a small green
pill, "Update ready · 0.1.1", with Restart, which installs it and quits for the new version
to start; amber when this copy must update to keep talking to that release. No dialog.

**Problem reports.** Settings › Report a problem: the person writes what happened and,
unless they untick it, adds the log of the last two days. The core and the window keep that
log in `diagnostics/` of the data folder (`crates/fogata-ffi/src/logs.rs`): one file per
day (UTC), `fogata-YYYY-MM-DD.log`, with what Fogata's crates say at `info` and above and
the libraries' warnings, at most 8 MiB a day, today's and the two days before kept;
`app-YYYY-MM-DD.log` beside it holds the window's errors (kept as long). The folder is its user's only
(0700, its files 0600 on Linux and macOS; on Windows the data folder's own permissions) and
never moves with the account (5e). What is personal never reaches it: each line is
scrubbed as it is formatted (`logs/scrub.rs`). The time, level, target and span names of a
line are fixed in the code and stay; a field keeps its value only when the value is a
number, a yes or no, a duration or one word of the code (an enum's variant), an address or
a peer keeps its shape with marks for its values (`/ip4/‹ip›/udp/‹port›/quic-v1/p2p/…k3A`),
an error's text is scrubbed like a message, and anything else becomes `‹value›`. A
message (its template and what was put in it cannot be told apart once formatted) loses
text in quotes of any kind (messages, whatever a library printed of data); the names of
people, groups, rooms and members known to the core, read again after its events, and the
names of the computer and its user; the paths of the data and home folders; the lines the
core writes with a name in them; links and Fogata invitations, e-mail addresses, IP
addresses and ports (plain and in multiaddrs), host names of the local network, paths and
file names (with the words of a name before its last), codes (a move's typed without its
dash too), keys (16 bytes of base64 too), long runs of hex (with or without `0x`), base32 or
base58 and lists of bytes, all
replaced by marks (`‹name›`, `‹ip›`…); peer ids keep their last six characters, device keys
and other long hex their first six, fingerprints their short form, so the steps of one
device can still be followed. Fogata's own crates log ids, never names. Standard error,
which a desktop session may keep in its journal, is scrubbed the same way at any level
`RUST_LOG` asks for; only a debug build with `FOGATA_RAW_LOGS` set leaves the values as they
are. A panic of the Rust side goes to the log as a scrubbed line, and standard error is
told only where in the code it happened (`logs::report_panics`), not what it said. A panic
in a call of the screens (from the reading of its arguments, which the bridge does before
the call runs, to the writing of its result), or in a task one waits for, reaches the window
with what it said scrubbed the same way (`bridge.rs`), since the window may print it; so does
any text of the core's for the window (why a download failed, a note, an update's or a
move's failure) that tells of a task's panic, however it was put into words (`bridge::told`).
The core's own errors for a task that panicked say only that (`fogata_net::task_failed`), and
the window says of any of them only that something went wrong (`explain`). The
window scrubs its errors before writing them. In a release build its console takes no
message either (`app/lib/src/console.dart`): an error the window did not catch (Flutter's,
or one that reached the root zone, which the engine would print whole) writes one line of its
kind and the place in the code, and goes to the day's log scrubbed; what the window and its
libraries print with `debugPrint` (the video library's `print`s too, the fork's `FOGATA.md`)
goes to the day's log alone. A debug build prints as Flutter does. A report is a
zip (`app/lib/src/report.dart`, at most 5 MB, the oldest lines going first): `report.txt`
(what was written, version and build, system and its version, language) and the log files
of the last 48 hours, scrubbed again line by line with the same rules and the names the
window knows, a second pass for anything a rule of the first missed in an older file
(`test/report_test.dart`). "Email it" writes the zip into the downloads folder,
opens the mail application (`mailto:` to the team's address, with a subject and the file's
name: what was written goes only in the file, since the address is handed to another
program on its command line) and shows the file in its folder to attach; "Save to a file"
writes it where the person chooses. Nothing is sent by Fogata itself. The application opens
only `https:` and `mailto:` addresses in other programs (`openExternal`). Settings › Donate to Fogata opens the
donation page in the browser (`kofiUrl` in `screens/donate.dart`).
