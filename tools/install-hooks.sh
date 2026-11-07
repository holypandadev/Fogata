#!/usr/bin/env bash
# Installs the repository's git hooks (tools/hooks/) into this clone, for every worktree of it:
#     tools/install-hooks.sh
# Run it again after a hook changes. A hooks folder set for this clone (git config --local
# core.hooksPath) is used when it is inside the clone; one set for every repository of the
# user (--global) is not, so the hook is never installed for other repositories.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd -P)
cd "$here"
top=$(cd "$(git rev-parse --show-toplevel)" && pwd -P)
common=$(cd "$(git rev-parse --git-common-dir)" && pwd -P)
hooks=$common/hooks
if configured=$(git config --local core.hooksPath); then
    # Relative to the top of the worktree, as git reads it.
    [[ $configured == /* ]] || configured=$top/$configured
    mkdir -p "$configured"
    configured=$(cd "$configured" && pwd -P)
    if [[ $configured != "$top"/* && $configured != "$common"/* ]]; then
        echo "core.hooksPath of this clone is $configured, outside it: not installing there" >&2
        exit 1
    fi
    hooks=$configured
elif global=$(git config core.hooksPath); then
    # Git would run the hooks of that folder, not this clone's: point this clone at its own.
    echo "core.hooksPath is $global for every repository; this clone gets its own hooks" \
        "folder (git config --local core.hooksPath $hooks)" >&2
    git config --local core.hooksPath "$hooks"
fi
mkdir -p "$hooks"
for hook in "$here"/hooks/*; do
    install -m 755 "$hook" "$hooks/$(basename "$hook")"
    echo "installed $(basename "$hook") in $hooks"
done
