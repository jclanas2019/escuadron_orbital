#!/usr/bin/env bash
# Monta la demo cinemática a partir de lo que grabó `NAVE_DEMO=carpeta nave_combate`:
#   bash herramientas/montar_demo.sh carpeta salida.mp4
# Necesita ffmpeg y python3 con numpy, scipy y soundfile.
set -euo pipefail
DEMO="$1"
OUT="${2:-escuadron_orbital_demo.mp4}"
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
FONT="$ROOT/fonts/DejaVuSansMono-Bold.ttf"
FPS=24
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

# 1) banda sonora
python3 "$HERE/mezcla_demo.py" "$DEMO" "$ROOT/assets" "$TMP/audio.wav"

# 2) instantes clave: fin del título (se pulsa ENTER a los 4 s) y comienzo del vuelo
#    (la música pasa de la pista del título a la de combate).
NFRAMES=$(ls "$DEMO/frames" | wc -l)
read -r FLY END < <(python3 - "$DEMO/audio.log" <<'EOF'
import sys
fly = None; end = None; last = 0.0
for l in open(sys.argv[1]):
    p = l.split()
    if p and p[0] == 'M':
        last = float(p[1])
        if fly is None and float(p[1]) > 0 and p[2] != '0':
            fly = float(p[1])
    if p and p[0] == 'END':
        end = float(p[1])
print(f"{fly or 0:.3f} {end or last:.3f}")
EOF
)
DUR=$(python3 -c "print(min($END, $NFRAMES / $FPS))")
TITLE_END=4.9
echo "vuelo desde ${FLY}s · fin ${DUR}s"

# 3) video completo con audio
ffmpeg -loglevel error -y -framerate $FPS -i "$DEMO/frames/f%05d.png" -i "$TMP/audio.wav" \
  -t "$DUR" -vf "scale=1920:1080:flags=lanczos,format=yuv420p" -c:v libx264 -preset veryfast -crf 16 \
  -c:a pcm_s16le "$TMP/full.mkv"

# 4) montaje: título (con fundido) + vuelo (fundido de entrada y de salida) + cartel final
B_DUR=$(python3 -c "print(round($DUR - $FLY, 3))")
ffmpeg -loglevel error -y -i "$TMP/full.mkv" -filter_complex "
 [0:v]trim=0:${TITLE_END},setpts=PTS-STARTPTS,fade=t=in:st=0:d=1.2,fade=t=out:st=$(python3 -c "print($TITLE_END-0.7)"):d=0.7[v1];
 [0:a]atrim=0:${TITLE_END},asetpts=PTS-STARTPTS,afade=t=in:st=0:d=1.2,afade=t=out:st=$(python3 -c "print($TITLE_END-0.7)"):d=0.7[a1];
 [0:v]trim=${FLY}:${DUR},setpts=PTS-STARTPTS,fade=t=in:st=0:d=0.6,fade=t=out:st=$(python3 -c "print(max(0,$B_DUR-1.5))"):d=1.5[v2];
 [0:a]atrim=${FLY}:${DUR},asetpts=PTS-STARTPTS,afade=t=in:st=0:d=0.4,afade=t=out:st=$(python3 -c "print(max(0,$B_DUR-2.5))"):d=2.5[a2];
 color=c=black:s=1920x1080:r=${FPS}:d=4,format=yuv420p,
   drawtext=fontfile=${FONT}:text='ESCUADRÓN ORBITAL':fontcolor=0xFFE8B0:fontsize=96:x=(w-tw)/2:y=(h-th)/2-50:alpha='min(1,t/0.8)',
   drawtext=fontfile=${FONT}:text='Hecho con Bevy':fontcolor=0xB0C8FF:fontsize=38:x=(w-tw)/2:y=(h)/2+60:alpha='min(1,max(0,(t-0.6)/0.8))',
   fade=t=out:st=3.2:d=0.8[v3];
 anullsrc=r=48000:cl=stereo,atrim=0:4[a3];
 [v1][a1][v2][a2][v3][a3]concat=n=3:v=1:a=1[v][a]" \
 -map "[v]" -map "[a]" -c:v libx264 -preset veryfast -crf 18 -pix_fmt yuv420p -c:a aac -b:a 192k -movflags +faststart "$OUT"
echo "Listo: $OUT"
