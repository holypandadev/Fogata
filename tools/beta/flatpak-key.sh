#!/usr/bin/env bash
# Makes the key that signs the Linux Flatpak's repository (tools/beta/flatpak.sh), once, in
# a GnuPG home of its own: an Ed25519 signing key that never expires, protected by a
# passphrase GnuPG asks for here and at every release. Testers' copies take from the
# loopback origin only commits this key signed, so it must stay the same for the life of
# the beta.
#
# The key is a root of trust of its own on Linux: flatpak installs whatever commit it signed
# from the loopback origin, through `flatpak update` or a software centre, with no release
# manifest and none of the release key's build rules (docs/architecture.md, 5f and 7). So it
# lives apart from the release and rescue keys, in a folder of its own (by default
# ~/.config/fogata-flatpak-key, never inside a folder that holds one of those keys, which is
# refused), with a passphrase of its own, and its copy goes on a drive of its own.
#
# Usage: tools/beta/flatpak-key.sh [--gpg-homedir <dir>] [--no-passphrase]
#   --gpg-homedir    where to keep it; by default ~/.config/fogata-flatpak-key/gnupg
#   --no-passphrase  a key with no passphrase, for the lab's tests only
set -euo pipefail
gpg_homedir=${FOGATA_GPG_HOMEDIR:-$HOME/.config/fogata-flatpak-key/gnupg}
no_passphrase=
while [[ $# -gt 0 ]]; do
    case $1 in
        --gpg-homedir) gpg_homedir=$2; shift 2 ;;
        --no-passphrase) no_passphrase=1; shift ;;
        *) echo "unknown option $1" >&2; exit 2 ;;
    esac
done

fail() {
    echo "flatpak-key.sh: $*" >&2
    exit 1
}

command -v gpg > /dev/null || fail "gpg is not installed"
# The Flatpak signing key is kept apart from the release and rescue keys (fogata release
# keygen), so that whoever takes one does not have the other: a GnuPG home inside a folder
# that holds one of them is refused.
beside_release_key() {
    local dir=$1
    [[ -d $dir ]] && dir=$(cd "$dir" && pwd -P)
    while :; do
        for file in release-key release-key.pub rescue-key rescue-key.pub; do
            [[ -e $dir/$file ]] && return 0
        done
        [[ $dir == / || $dir == . || -z $dir ]] && return 1
        dir=$(dirname "$dir")
    done
}
beside_release_key "$gpg_homedir" &&
    fail "$gpg_homedir is inside a folder that holds a release or rescue key: keep the Flatpak signing key in a folder of its own"
if [[ -d $gpg_homedir ]] &&
    gpg --homedir "$gpg_homedir" --batch --list-secret-keys --with-colons 2>/dev/null | grep -q '^sec'; then
    fail "a signing key is already in $gpg_homedir"
fi
mkdir -p "$gpg_homedir"
chmod 700 "$gpg_homedir"
if [[ -n $no_passphrase ]]; then
    gpg --homedir "$gpg_homedir" --batch --quiet --gen-key <<'EOF'
%no-protection
Key-Type: EDDSA
Key-Curve: ed25519
Key-Usage: sign
Name-Real: Fogata beta
Expire-Date: 0
%commit
EOF
else
    echo "GnuPG asks for the passphrase that protects the signing key (and again at every release)."
    gpg --homedir "$gpg_homedir" --quiet --quick-gen-key "Fogata beta" ed25519 sign never
fi
fingerprint=$(gpg --homedir "$gpg_homedir" --batch --list-secret-keys --with-colons |
    awk -F: '$1 == "fpr" {print $10; exit}')
[[ -n $fingerprint ]] || fail "the key was not made"
gpg --homedir "$gpg_homedir" --batch --export "$fingerprint" > "$gpg_homedir/../flatpak-signing-key.gpg"
gpgconf --homedir "$gpg_homedir" --kill gpg-agent 2> /dev/null || true
echo "Made the Flatpak signing key $fingerprint in $gpg_homedir."
echo "Its public half is in $(cd "$gpg_homedir/.." && pwd)/flatpak-signing-key.gpg; every first install carries it."
echo "Keep a copy of the folder offline, on a drive of its own: a copy installed with this key takes releases signed with no other, and whoever has it and its passphrase can make Linux copies install their code."
