# The device-echo.sh programs of the call lab's measurements of WebRTC patches 0015 and 0016:
# `. tools/call-lab/device-echo-cases.sh <case>` sets them, then `tools/call-lab/device-echo.sh <case>`.
# pulse, sinkmove, srcmove, unplug: the sound server and its streams (patch 0015); jump: the
# echo's delay jumping with nothing marked (A8); a1 a2 b c1 c2
# d1 d2: an unmute after a change of devices (patch 0016), Q = FogataQuietOut, O = FogataOther.
export SETTLE=20 CYCLE=20 CYCLES=${CYCLES:-6} LOOP_Q=60 LOOP_O=180 LOOP_DEFAULT=none START_OUTPUT=FogataQuietOut-laba
case $1 in
  pulse) export CYCLE=25 PROG_EVEN="0:pulse 0:E:back" ;;
  chosen) export CYCLE=30 PROG_EVEN="0:pulselate" ;;
  sinkmove) export PROG_EVEN="0:movesink:laba_other 0:E:moved" PROG_ODD="0:movesink:laba_quiet 0:E:moved" ;;
  srcmove) export PROG_EVEN="0:movesrc:laba_mic2 0:E:moved" PROG_ODD="0:movesrc:laba_mic 0:E:moved" ;;
  # A8: the echo's delay jumps (60 to 180 ms and back) with nothing marked.
  jump) export PROG_EVEN="0:jump:180 0:E:jump" PROG_ODD="0:jump:60 0:E:jump" ;;
  unplug) export LOOP_DEFAULT=120 PROG_EVEN="0:unplug 0:E:unplug 10:plug 10:E:plug" ;;
  # L2: the unmute after a change of devices. Q = FogataQuietOut-laba, O = FogataOther-laba.
  a1|a2) [ "$1" = a2 ] && export LOOP_O=60
    export PROG_EVEN="0:out:FogataOther-laba 6:mic:off 9:mic:on 9:E:unmute 16:words:ref"
    export PROG_ODD="0:out:FogataQuietOut-laba 6:mic:off 9:mic:on 9:E:unmute 9.5:words:dt0.5 12.5:words:dt3.5 16:words:ref" ;;
  b) export PROG_EVEN="0:mic:off 3:out:FogataOther-laba 9:mic:on 9:E:unmute 16:words:ref"
    export PROG_ODD="0:mic:off 3:out:FogataQuietOut-laba 9:mic:on 9:E:unmute 9.5:words:dt0.5 12.5:words:dt3.5 16:words:ref" ;;
  c1) export CYCLE=30 LOOP_Q=none LOOP_O=60
    export PROG_EVEN="0:out:FogataQuietOut-laba 12:mic:off 14:out:FogataOther-laba 18:mic:on 18:E:unmute 26:words:ref"
    export PROG_ODD="0:out:FogataQuietOut-laba 12:mic:off 14:out:FogataOther-laba 18:mic:on 18:E:unmute 18.5:words:dt0.5 21.5:words:dt3.5 26:words:ref" ;;
  c3) export CYCLE=30 LOOP_Q=none LOOP_O=60
    export PROG_EVEN="0:out:FogataQuietOut-laba 14:out:FogataOther-laba 14:E:change 26:words:ref"
    export PROG_ODD="0:out:FogataQuietOut-laba 14:out:FogataOther-laba 14:E:change 14.5:words:dt0.5 17.5:words:dt3.5 26:words:ref" ;;
  c2) export CYCLE=30 LOOP_Q=none LOOP_O=60 START_OUTPUT=FogataOther-laba
    export PROG_EVEN="0:out:FogataOther-laba 12:mic:off 14:out:FogataQuietOut-laba 18:mic:on 18:E:unmute 18.5:words:dt0.5 21.5:words:dt3.5 26:words:ref" ;;
  d1) export LOOP_O=60
    export PROG_EVEN="0:mic:off 3:in:FogataMicTwo-laba 9:mic:on 9:E:unmute 16:words:ref"
    export PROG_ODD="0:mic:off 3:in:FogataMic-laba 9:mic:on 9:E:unmute 9.5:words:dt0.5 12.5:words:dt3.5 16:words:ref" ;;
  d2) export LOOP_O=60
    export PROG_EVEN="0:in:FogataMicTwo-laba 6:mic:off 9:mic:on 9:E:unmute 16:words:ref"
    export PROG_ODD="0:in:FogataMic-laba 6:mic:off 9:mic:on 9:E:unmute 9.5:words:dt0.5 12.5:words:dt3.5 16:words:ref" ;;
  *) echo "unknown case $1" >&2; return 1 ;;
esac
