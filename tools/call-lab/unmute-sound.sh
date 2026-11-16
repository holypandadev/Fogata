#!/usr/bin/env bash
# The sound of a room for mic.sh unmute, made with ffmpeg's filters (nothing recorded or
# downloaded), 48 kHz, mono, 16 bits:
#   <folder>/words.wav   a few words, 2.1 s: three words of two or three syllables, a
#                        voice whose pitch falls in each syllable (a sawtooth of 125 to
#                        160 Hz through three formants), as loud as UNMUTE_WORDS_DB says
#                        (dBFS, RMS over the 2.1 s; default -28)
#   <folder>/room.sh     prints the room's steady noise to standard output, without end, in
#                        real time (s16le): a fan (brown noise from 40 Hz to 800 Hz), its
#                        motor's hum (100 Hz and its harmonics, a slow beat at 29 Hz) and a
#                        hiss above 3 kHz, as loud as UNMUTE_NOISE_DB says (dBFS, RMS;
#                        default -52)
# Usage: tools/call-lab/unmute-sound.sh <folder>
set -euo pipefail
out=$1
mkdir -p "$out"
command -v ffmpeg > /dev/null || { echo "unmute-sound.sh needs ffmpeg" >&2; exit 1; }
noise_db=${UNMUTE_NOISE_DB:--52}
words_db=${UNMUTE_WORDS_DB:--28}

# The RMS of a file in dBFS, as ffmpeg's volumedetect gives it.
rms() {
  ffmpeg -hide_banner -nostats -i "$1" -af volumedetect -f null - 2>&1 |
    sed -n 's/.*mean_volume: \(-\{0,1\}[0-9.]*\) dB.*/\1/p'
}

# The room: three sources mixed, then brought to the level asked.
room_graph() { # seconds (0: without end)
  local d=""
  [ "$1" != 0 ] && d=":duration=$1"
  local ed=""
  [ "$1" != 0 ] && ed=":d=$1"
  echo "anoisesrc=color=brown:amplitude=0.6:sample_rate=48000:seed=7$d,highpass=f=40,lowpass=f=800[fan];" \
    "anoisesrc=color=white:amplitude=0.03:sample_rate=48000:seed=11$d,highpass=f=3000[hiss];" \
    "aevalsrc='0.05*sin(2*PI*100*t)+0.025*sin(2*PI*200*t+1)+0.012*sin(2*PI*300*t+2)+0.006*sin(2*PI*29*t)':s=48000$ed[hum];" \
    "[fan][hiss][hum]amix=inputs=3:normalize=0,aformat=sample_fmts=s16:channel_layouts=mono"
}
ffmpeg -hide_banner -loglevel error -y -filter_complex "$(room_graph 20)" "$out/room-sample.wav"
room_gain=$(python3 -I -c "print(round($noise_db - ($(rms "$out/room-sample.wav")), 2))")
cat > "$out/room.sh" << EOF
#!/usr/bin/env bash
exec ffmpeg -hide_banner -loglevel error -filter_complex "$(room_graph 0),volume=${room_gain}dB,arealtime" -f s16le -ar 48000 -ac 1 -
EOF
chmod +x "$out/room.sh"
ffmpeg -hide_banner -loglevel error -y -filter_complex "$(room_graph 20),volume=${room_gain}dB" "$out/room-check.wav"

# The words: syllables (start, length, pitch), each a sawtooth whose pitch falls 15 %
# over it, under a sine-squared envelope.
syllables="0.00 0.18 150|0.20 0.22 135|0.65 0.15 160|0.82 0.25 140|1.35 0.16 155|1.53 0.14 145|1.70 0.30 125"
expr=""
IFS='|' read -r -a parts <<< "$syllables"
for s in "${parts[@]}"; do
  read -r t d f <<< "$s"
  x="(t-$t)"
  term="between(t,$t,$t+$d)*pow(sin(PI*$x/$d),2)*(2*mod($f*$x-$f*0.15*$x*$x/(2*$d),1)-1)"
  expr=${expr:+$expr+}$term
done
words_graph="aevalsrc='0.5*($expr)':s=48000:d=2.1,highpass=f=90,lowpass=f=4000,equalizer=f=600:t=q:w=3:g=8,equalizer=f=1300:t=q:w=4:g=6,equalizer=f=2600:t=q:w=4:g=4,aformat=sample_fmts=flt:channel_layouts=mono"
ffmpeg -hide_banner -loglevel error -y -filter_complex "$words_graph" -c:a pcm_f32le "$out/words-raw.wav"
words_gain=$(python3 -I -c "print(round($words_db - ($(rms "$out/words-raw.wav")), 2))")
ffmpeg -hide_banner -loglevel error -y -i "$out/words-raw.wav" -af "volume=${words_gain}dB,aformat=sample_fmts=s16" "$out/words.wav"
echo "room: $(rms "$out/room-check.wav") dBFS RMS (asked $noise_db, gain ${room_gain} dB); words: $(rms "$out/words.wav") dBFS RMS (asked $words_db)" | tee "$out/sound.txt"
rm -f "$out/words-raw.wav" "$out/room-sample.wav" "$out/room-check.wav"
