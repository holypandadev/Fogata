#!/usr/bin/env bash
# Installs Ubuntu packages on one of GitHub's computers, whatever its mirror of Ubuntu does
# that day. Usage: tools/ci-apt.sh <package>...
#
# GitHub's image takes Ubuntu's packages from the mirrors listed in /etc/apt/apt-mirrors.txt,
# the first one first. When that one does not answer (its name not resolving, or resolving
# to nothing that answers), or answers and then gives a few packages a minute, apt neither
# gives up nor goes on to the next in good time: one job sat there for two hours, and on
# another evening every job ran into its 20 minutes with two thirds of the packages
# fetched. So the fetching has a time of its own here (two minutes for the lists, four for
# the packages, about ten times what they take), and when it runs out the same is asked of
# another mirror, alone in the list: what was fetched already is kept, so each try has less
# left to fetch. The packages are installed once all of them are there.
#
# CI_APT_ARCHIVES=<folder>, when set, is where the packages' archives (.deb) are fetched and
# kept instead of apt's own /var/cache/apt/archives, which is root's: a workflow keeps that
# folder between runs (actions/cache, run as the runner's user), so that a run fetches only
# what is new since and a slow mirror costs little. The folder is made if missing, with the
# partial/ apt fetches into (apt hands that one to its own unprivileged user, _apt). apt takes
# an archive it finds there by its size alone, without its hashes (pkgAcqArchive), so before
# anything is fetched every archive already there is checked against the lists just fetched,
# which Ubuntu signs: a file is kept only when it is a plain file and the lists give its
# SHA-256 for its own name (<package>_<version, ':' written %3a>_<architecture>.deb, as apt
# names it); anything else is deleted, and apt fetches what is missing. That deletes the
# versions the lists no longer give too; a release's own version stays in the lists beside
# its update, so at most one more version of a package may be kept. Without it, nothing
# changes.
set -uo pipefail

list=/etc/apt/apt-mirrors.txt
others=(http://archive.ubuntu.com/ubuntu/ http://azure.archive.ubuntu.com/ubuntu/
  http://us.archive.ubuntu.com/ubuntu/ http://archive.ubuntu.com/ubuntu/)
apt=(apt-get -o Acquire::Retries=3 -o Acquire::http::Timeout=30 -o Acquire::https::Timeout=30)
archives=${CI_APT_ARCHIVES:-}

[[ $# -gt 0 ]] || { echo "usage: $0 <package>..." >&2; exit 2; }
if [[ -n $archives ]]; then
  mkdir -p "$archives/partial" || exit 1
  apt+=(-o "Dir::Cache::Archives=$archives/")
fi

# Deletes everything in $archives named *.deb but the plain files whose name and SHA-256 the
# lists give together. Only names that are package names are asked of apt-cache.
check_archives() {
  local debs file name names=() known='' sum kept=0 deleted=0
  shopt -s nullglob dotglob
  debs=("$archives"/*.deb)
  shopt -u nullglob dotglob
  for file in "${debs[@]}"; do
    name=${file##*/}
    name=${name%%_*}
    [[ $name =~ ^[a-z0-9][a-z0-9+.-]*$ ]] && names+=("$name")
  done
  if (( ${#names[@]} > 0 )); then
    # "<file name apt gives the archive> <sha256>" for each version the lists give.
    known=$(apt-cache show -- "${names[@]}" 2> /dev/null | awk '
      function out() {
        if (p != "" && v != "" && a != "" && s != "") { gsub(/:/, "%3a", v); print p "_" v "_" a ".deb " s }
        p = v = a = s = ""
      }
      /^Package: / { p = $2 } /^Version: / { v = $2 } /^Architecture: / { a = $2 } /^SHA256: / { s = $2 }
      /^$/ { out() } END { out() }')
  fi
  for file in "${debs[@]}"; do
    if [[ -f $file && ! -L $file ]] && sum=$(sha256sum < "$file" 2> /dev/null) &&
      grep -qxF -- "${file##*/} ${sum%% *}" <<< "$known"; then
      kept=$((kept + 1))
    else
      sudo rm -rf -- "$file"
      deleted=$((deleted + 1))
    fi
  done
  echo "archives kept from before: $kept as the lists give them, $deleted deleted"
}

[[ -f $list ]] && echo "mirrors: $(tr -s '\t\n' ' ' < "$list")"
fetched=
checked=
for try in 0 1 2 3 4; do
  if (( try > 0 )); then
    mirror=${others[try - 1]}
    echo "::warning::Ubuntu's packages did not come in time (try $try); asking $mirror"
    [[ ! -f $list ]] || printf '%s\tpriority:1\n' "$mirror" | sudo tee "$list" > /dev/null
  fi
  sudo timeout 120 "${apt[@]}" update || continue
  if [[ -n $archives && -z $checked ]]; then
    check_archives
    checked=1
  fi
  if sudo timeout 240 "${apt[@]}" install -y --no-install-recommends --download-only "$@"; then
    fetched=1
    break
  fi
done
[[ -n $fetched ]] || { echo "::error::no mirror of Ubuntu gave the packages"; exit 1; }
sudo "${apt[@]}" install -y --no-install-recommends "$@"
