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
set -uo pipefail

list=/etc/apt/apt-mirrors.txt
others=(http://archive.ubuntu.com/ubuntu/ http://azure.archive.ubuntu.com/ubuntu/
  http://us.archive.ubuntu.com/ubuntu/ http://archive.ubuntu.com/ubuntu/)
apt=(apt-get -o Acquire::Retries=3 -o Acquire::http::Timeout=30 -o Acquire::https::Timeout=30)

[[ $# -gt 0 ]] || { echo "usage: $0 <package>..." >&2; exit 2; }
[[ -f $list ]] && echo "mirrors: $(tr -s '\t\n' ' ' < "$list")"
fetched=
for try in 0 1 2 3 4; do
  if (( try > 0 )); then
    mirror=${others[try - 1]}
    echo "::warning::Ubuntu's packages did not come in time (try $try); asking $mirror"
    [[ ! -f $list ]] || printf '%s\tpriority:1\n' "$mirror" | sudo tee "$list" > /dev/null
  fi
  if sudo timeout 120 "${apt[@]}" update &&
    sudo timeout 240 "${apt[@]}" install -y --no-install-recommends --download-only "$@"; then
    fetched=1
    break
  fi
done
[[ -n $fetched ]] || { echo "::error::no mirror of Ubuntu gave the packages"; exit 1; }
sudo "${apt[@]}" install -y --no-install-recommends "$@"
