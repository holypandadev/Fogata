Part of [Fogata architecture](../architecture.md).

## 5f. Updates of pre-releases (fogata-update, fogata-net `updates`, fogata-core `updates`)

Pre-releases (the `beta` channel) travel over Fogata itself, from friend to friend, with no
server. A malicious update would reach every tester, so a copy takes
only what the release key signed.

**The release.** A manifest (JSON) names the product and channel (`dev.holypanda.fogata`,
`beta`), a build number that goes up with every release, the version people see, the oldest
build that still works with it (`min_build`: below it a copy is told it must update, for a
change of the network protocol), when it was made and when it expires (180 days by
default), notes, and one package per platform (`linux-flatpak`, `windows-x64`,
`macos-arm64`, `macos-x64`) with its file name, size, BLAKE3 and SHA-256, and the BLAKE3 of
every 1 MiB chunk. The universal Mac app is one package listed under both Mac platforms (the
same file, size and hashes), as a copy takes the platform of the process it runs as. The release key (Ed25519) signs `fogata/v1/release-manifest\0` followed by
the manifest's exact bytes; the public keys a build trusts are compiled into it
(`RELEASE_KEYS`, read at run time from the keys marker of the library, which the release
tooling checks in every package together with its build and the commit it was built from),
never read from the data folder or a package. A copy checks size (256 KiB
at most), key and signature before it parses anything, then every field: a format it knows,
plain names, a package of at most 400 MB, chunk hashes that match the size. It takes a
release only when its build is above every build it runs or has (no downgrade, no replay)
and at most 1000 above the build it runs (`MAX_BUILD_STEP`), it was made no earlier than the
release it holds, it has not expired, and it was not made more than a day after the copy's
clock. The secret
half of the key stays in `~/.config/fogata-release/` on the publisher's computer (made by
`fogata release keygen`, readable by its owner only, and sealed with a passphrase: Argon2id
with a random salt, 128 MiB and three passes, then XChaCha20-Poly1305 under the label
`fogata/v1/release-key`, so a copy of the file alone is no key; `--plain` leaves it in hex
for a computer kept offline). Every build trusts two keys or none (a test checks it): a
release key, and a rescue key made the same way (`keygen --rescue`) on a drive kept offline.
A release the rescue key signs may be up to 2000 builds above the build a copy runs and is
not held to the time of the release held, so when a release key leaks or is lost, a release
numbered 1001 above the last good build, signed with the rescue key, is taken over anything
the leaked key can have made a copy hold (at most 1000 above its build), and the build in it
trusts a new release key in place of the old; nothing the leaked key signs passes it after.
Rotating a key that did not leak is a release that adds the new public key, then one that
drops the old. There is no revocation list: a copy that installed a release of a leaked key
runs that code. The first installs, the files of a pre-release, are listed with their SHA-256 in
`SHA256SUMS`, signed by the same key as an SSH signature (namespace `file`: `SSHSIG`, the
namespace and the file's SHA-512, a message no manifest signature covers), which `fogata
release verify-sums` (which also fails on a file the list does not name) and `ssh-keygen -Y
verify -I fogata-prerelease` check (the pre-release's keys go by that name in allowed signers
files, the SSH key of public releases by `fogata-release`, so neither passes for the other);
testers check that signature with the key lines of `tools/allowed_signers`, taken from the
source code apart from the download, and compare the checksum with their file's, since the
builds have no paid code signing.

**Spreading** (`/fogata/update/1`). A device asks each friend (recognised on its connection)
and each verified member of a shared group that lists the protocol, when it connects and
every 20 minutes, for a release newer than the build it has, naming its platform; the answer
is the manifest with its key and signature and the platforms whose packages that device
holds complete. Anyone else's stream is closed before anything is read from it and nothing
about releases goes to the DHT, so only friends and members learn a device's build. A newer, good release is kept
in `updates/` (`release.json`, `release.sig`) and the package for this platform fetched from
a device that has it, from the first missing chunk to the end; each chunk is checked against
its hash as it arrives and written to `updates/<platform>.pack.part` (made new, owner only,
never through a link), so a damaged chunk ends the download at once, keeping the good ones,
and that device is not asked again for an hour. A download that stops goes on from its last
whole chunk. The whole package is hashed again (BLAKE3 and SHA-256) before it becomes
`<platform>.pack`. Then the device tells its friends and members it has the build (`HAVE`,
which only makes them ask it, at most every 30 seconds) and gives the release on: so it
spreads to a friend of a friend who is not the publisher's friend. A device fetches only its
own platform's package, and another platform's only when a friend or member asked it for
that one (it then fetches it from whoever has it, one at a time), so a Windows friend behind
a Linux friend gets it too; it keeps one such package at most (one nobody asked for in an
hour makes way for another), lets it go once nobody has asked for it in two days, and
fetches it only while its own and it stay under 600 MiB together, so a friend cannot fill
its disk with every platform's. A package starts to come only once the disk has room for
what of it is still to come and 256 MiB to spare, as a download does; until then the
update's status says the disk is why, and it is tried again at the next step. The publisher's copy (`fogata release publish` marks it)
keeps every platform's package. A device uploads to three devices at once at most, one stream
per device (a friend or member may connect with several sessions, counted together), 4 MB/s
in all, and 1 GiB to one device in a day (a package whose rest does not fit in the day is
not started: the device is told to ask later and goes on from where it stopped); asks from one device are answered at most every 3
seconds, and so are its words that it has a build and its fetches of a package that is not
there. A friend or member has at most 4 requests answered at once, and all together 32; a
stream past that is closed unread, and a request that has not arrived whole 10 s after its
stream came is closed too. An upload whose reader took less than 64 KiB/s over a minute of
waiting for it (time spent on the disk or the 4 MB/s pace not counted, as for files above:
`slow.rs`) ends while another device turned away for want of a place in the last two minutes
may be waiting (the device cut off goes on from its last whole chunk when it asks again, 45 s
later); a slow line keeps its place while nobody waits. An ask, a word that a device has a
build, or a fetch whose stream ends or is reset before any answer is made again, after 0.1,
0.3 and 1 s, as a file request is: friends that hear of a release at once all ask its
holder at once, and copies before 0.1.0-beta.9 drop all but one of such a burst (see the
`stream` behaviour). The publisher
puts a release into a data folder of his (`fogata release publish`, which checks every byte
first and refuses a release its own build would let go at its next start: older, too far
above, expired), and the copy running there gives it like any other.

**From the releases page.** Besides friends and members, a copy reads the newest releases of
the project's repository on GitHub (`RELEASES_API`, `RELEASES_REPO` in `fogata-update`; the
API's list of releases, drafts left out) soon after it starts, when the splash asks, and every
six hours (every fifteen minutes after the page could not be read): of the first three that
carry `release.json` and `release.sig` it fetches those two files (256 KiB and 1 KiB at most,
4 MiB for the list) and takes the newest by exactly the rules above, as from a friend: the
page decides nothing a friend could not. Its package for this platform is the asset with the
file name the manifest gives, fetched over HTTPS (rustls, the system's certificate
authorities) into the same `<platform>.pack.part`, each 1 MiB chunk checked against its hash
as it arrives, a stopped download going on from the last whole chunk with a range request;
a damaged chunk ends it, and that release is fetched from friends and members after that.
A download that brings less than 16 KiB/s over two minutes, or nothing for one, stops and
goes on two minutes later; the small files must arrive within five minutes.
The page is used before friends and members when it has the package, which spares their
uploads; a package from there is given on like any other. A copy that trusts no release key
does not read the page (nothing could pass), nor do the network lab's copies
(`FOGATA_UPDATE_WEB=off`, which `fogata update run --web` also takes, or another API's address
and repository, for tests). GitHub learns the device's address, that it runs Fogata (the
`User-Agent` names the version) and when it looks: the only server Fogata talks to of its own.

Only the newest release is kept, with its packages; one older than the running build goes at
start. Nothing installs without the application asking `install_update`: at start, while
its splash shows, when a release is ready or comes while it waits (5d, The splash: a few
seconds for friends and the releases page to answer, then the download as long as it moves,
three minutes at most unless this copy must update);
later, when the person presses Restart in the title bar's pill. The state the screens read (`update_status`): up to date,
checking, downloading (bytes), verifying, ready, installing, failed (no package for this
platform, disk, cannot install here, install failed, an install that did not take) and
whether this copy must update, and while it downloads, from which friend's or member's
device. `fogata update status|run|install` show and do the same from the command line.

**Installing.** The package is hashed once more, unpacked (the package format refuses
absolute paths, `..`, names that differ only in case or that Windows reads otherwise, links
that lead out, and writes into a folder made new, owner only, never through a link), then:

- *Linux (Flatpak)*: the package is an OSTree repository holding the new version, its
  commit and summary signed (GnuPG, an Ed25519 key of its own made once by
  `tools/beta/flatpak-key.sh`, under a passphrase GnuPG asks for, in a folder of its own
  apart from the release and rescue keys: the scripts refuse a GnuPG home in a folder that
  holds one of them, and `fogata release keygen` a folder that holds it). The first install
  (`flatpak install --user Fogata.flatpak`) carries that key's public half and names
  `http://127.0.0.1:47811/` as the application's origin, which flatpak then verifies
  (`gpg-verify`, `gpg-verify-summary`): from that address it takes only a summary and a
  commit this key signed, and no commit older than the one installed (it compares their
  times; `tools/beta/test-flatpak-origin.sh` tries each case). Any program of the computer
  can answer on that fixed loopback port while Fogata is not serving there (another
  account, a Flatpak with the network), and `flatpak update` or a software centre then
  fetches from it; without the key it can offer only versions this key signed that are
  newer than the one installed, which flatpak installs although Fogata itself might not
  (one whose release expired, say). Flatpak knows nothing of the release key, the manifest
  or its build rules, and installs through the portal whatever the remote's newest commit
  is (the portal names no commit): it is this key, not the release key, that decides what
  flatpak installs on Linux. A commit it signs installs with no manifest at all, and a
  summary it signs can make flatpak trust further keys (`xa.gpg-keys`, added to the old,
  never in place of them) or fetch from another address (`xa.redirect-url`), for good. To
  install, the application serves the unpacked repository there, on the
  loopback interface only and to requests that name that address, first asks the port for
  the repository's `config` to see that its own server, and no other program, answers, and
  then asks the Flatpak portal to update it (`UpdateMonitor.Update`). The portal asks the
  person once whether Fogata may update itself, checks the application id and refuses a
  version that asks for more permissions, so the sandbox stays whole (the alternative,
  `flatpak-spawn --host` with `--talk-name=org.freedesktop.Flatpak`, would let any bug in
  the application run anything on the computer). The new version is started through the
  portal (`Spawn`, newest version, no arguments) and this one quits. Other programs that
  update Flatpaks find nothing at that address while no update installs, which flatpak
  treats as a warning for that application only.
- *Windows*: the application lives in `%LOCALAPPDATA%\Programs\Fogata` (installed for one
  person, no administrator, by the first install's script, which copies only the files
  `Fogata.sha256` in the download lists, checks each copy's SHA-256 and clears the mark of
  the web of those files alone). The package is unpacked beside
  it as `Fogata.new`; a hidden PowerShell waits for the application to end, moves the old
  folder aside, puts the new one in its place and starts it, or puts the old one back and
  starts it if a step fails. Files Fogata writes itself carry no mark of the web, so
  SmartScreen is not shown again (the manifest's signature stands for the files instead).
- *macOS*: the sandboxed application cannot replace its own bundle, so it unpacks the new
  `Fogata.app` into `updates/` in its container and opens
  `Contents/Library/Helpers/Fogata Updater.app`, signed by the same team without the
  sandbox. A sandboxed application cannot pass arguments to an application it opens (Launch
  Services drops them), so the helper finds both itself: the new bundle at the one place Fogata
  unpacks it, and Fogata among the running copies of the installed bundle; tests run outside
  the sandbox may name both as arguments, checked the same way. Anyone can open the helper with
  arguments, so it believes them only as far as it checks them: the installed bundle is
  the one it is inside of; the new bundle must be at `.../updates/Fogata.app` inside
  Fogata's container (`~/Library/Containers/dev.holypanda.fogata/Data`, pinned in at build
  time; the path as given must be the path resolved, so no link leads elsewhere); the
  process must be running and be the installed bundle's. It waits for Fogata to end (Fogata
  stays two seconds after opening it), copies the new bundle to a folder of its own on the
  installed bundle's volume that nothing else can write, and checks that copy
  (`app/macos/Updater/Checks.swift`): a valid signature, for `dev.holypanda.fogata`, by the
  team pinned in at build time (`tools/beta/macos-updater.sh --team`; so the permissions
  given to Fogata stay), with the App Sandbox entitlement (so no debug or profile build of
  the team, which run outside it), and a build, as the Info.plist the signature seals says
  (`release.py` checks it is the release's), above the installed bundle's and at most 2000
  (`MAX_RESCUE_STEP`) above it. Fogata applies the release's build rules before it
  downloads anything, but whatever can write its container, Fogata itself included, could
  put an older bundle of the team there: the helper refuses it on its own. Then it
  clears the quarantine of that copy, swaps it in and opens it; on any failure after the
  copy it removes the new bundle and opens the old one, and on bad arguments it touches
  nothing. The copy's own folder is made readable by every user (Fogata unpacked it
  readable by its user only) before the swap, so every user of the Mac can open it.
  Signatures carry Apple's timestamp (`tools/sign-macos.sh`), so they stay valid
  when the Apple Development certificate expires after its year.

Each leaves `window/restart` so the new copy waits for the old one to let go of the data
folder, and `updates/installed.json`: a copy that starts still older than the build it
installed says so (the install did not take) rather than offering it again. The data folder
is never touched; later pre-releases open the data of earlier ones as every version does.
