# Sourced by the build scripts. The library keeps the names of its source files as the
# build folder sees them (in its log and its failed checks), and GN resolves links: only
# fetch.sh's layout, <work>/webrtc/src and <work>/out/<name>, the same on every computer,
# gives the same bytes from the same sources (README.md, "Checking an archive").

# Stops unless build folder $1 sees the sources ($src) as ../../webrtc/src.
same_layout() {
    local seen
    seen=$(python3 -I -c 'import os, sys; print(os.path.relpath(*map(os.path.realpath, sys.argv[1:])))' \
        "$src" "$1")
    [[ $seen == ../../webrtc/src ]] ||
        { echo "$1 sees the sources as $seen, not ../../webrtc/src: use fetch.sh's layout" >&2; exit 1; }
}
