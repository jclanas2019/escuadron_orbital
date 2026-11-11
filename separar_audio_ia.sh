#!/usr/bin/env bash
# ============================================================================
#  Separación de audio con IA (Demucs) — opcional, para mejor calidad.
#  Separa una canción en batería, bajo, voces y "otros" (sintes, efectos)
#  y deja los resultados en .ogg dentro de assets/audio/musica/stems_ia/.
#
#  Uso:   ./separar_audio_ia.sh  cancion.mp3
#  Requiere: python3 y ffmpeg  (en Mac:  brew install python ffmpeg)
#  La primera vez descarga Demucs (~2 GB con PyTorch) en .venv_audio/
# ============================================================================
set -euo pipefail
cd "$(dirname "$0")"
IN="${1:?Indica el archivo de audio: ./separar_audio_ia.sh cancion.mp3}"
command -v ffmpeg >/dev/null || { echo "Falta ffmpeg (Mac: brew install ffmpeg)"; exit 1; }
if [ ! -d .venv_audio ]; then
    python3 -m venv .venv_audio
    .venv_audio/bin/pip install --upgrade pip
    .venv_audio/bin/pip install demucs soundfile
fi
OUT=$(mktemp -d)
echo "▶ Separando con Demucs (htdemucs)… puede tardar unos minutos"
.venv_audio/bin/python -m demucs -n htdemucs -o "$OUT" "$IN"
DEST=assets/audio/musica/stems_ia
mkdir -p "$DEST"
for f in "$OUT"/htdemucs/*/*.wav; do
    n=$(basename "$f" .wav)
    case $n in
        drums) es=bateria ;; bass) es=bajo ;; vocals) es=voces ;; *) es=otros_sintes_efectos ;;
    esac
    ffmpeg -hide_banner -loglevel error -y -i "$f" -c:a libvorbis -q:a 5 "$DEST/$es.ogg"
    echo "  ✓ $DEST/$es.ogg"
done
rm -rf "$OUT"
echo "Listo. Para usar una pista como música del juego, cópiala sobre"
echo "assets/audio/musica/musica_completa.ogg (combate) o musica_titulo.ogg (título)."
