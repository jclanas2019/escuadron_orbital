"""Prepara las capas de música adaptativa a partir de una pista (Battle Stations).
Uso: python3 musica_adaptativa.py pista.mp3 carpeta_salida
Corta secciones en compases, las hace bucle sin saltos y separa la batalla en
armonía + percusión (HPSS) para poder subir la percusión según el peligro."""
import sys, os, numpy as np, librosa, soundfile as sf
src, out = sys.argv[1], sys.argv[2]
os.makedirs(out, exist_ok=True)
SR = 44100
y, _ = librosa.load(src, sr=SR, mono=False)
mono = y.mean(0)
tempo, beats = librosa.beat.beat_track(y=mono, sr=SR, units='time')
beats = np.asarray(beats)
def snap(t):
    return float(beats[np.argmin(np.abs(beats - t))])
def loop(seg, xf):
    """Bucle sin clic: la cola se funde con el principio."""
    n = int(xf * SR)
    a = seg[:, :-n].copy()
    w = np.linspace(0, 1, n)
    a[:, :n] = a[:, :n] * w + seg[:, -n:] * (1 - w)
    return a
def cut(t0, t1):
    a, b = snap(t0), snap(t1)
    return y[:, int(a * SR):int(b * SR) + int(1.0 * SR)], a, b
def save(name, x):
    x = x / (np.abs(x).max() + 1e-9) * 0.89
    sf.write(os.path.join(out, name + '.wav'), x.T.astype(np.float32), SR)
    print(name, f'{x.shape[1] / SR:.2f}s')
# suspenso: la sección tranquila y tensa
s, a, b = cut(52.5, 74.5)
save('bs_suspenso', loop(s, 1.0))
# batalla: sección principal, separada en armonía y percusión (mismo largo, en sincronía)
s, a, b = cut(9.3, 50.8)
H, P = [], []
for ch in s:
    D = librosa.stft(ch, n_fft=4096, hop_length=1024)
    h, p = librosa.decompose.hpss(D, margin=(1.0, 1.5))
    H.append(librosa.istft(h, hop_length=1024, length=len(ch)))
    P.append(librosa.istft(p + (D - h - p) * 0.5, hop_length=1024, length=len(ch)))
H, P = np.array(H), np.array(P)
k = max(np.abs(H).max(), np.abs(P).max())
for name, x in [('bs_batalla_armonia', H), ('bs_batalla_percusion', P)]:
    x = loop(x, 1.0) / k * 0.89
    sf.write(os.path.join(out, name + '.wav'), x.T.astype(np.float32), SR)
    print(name, f'{x.shape[1] / SR:.2f}s')
# crescendo: el tramo que sube hasta los golpes
s, a, b = cut(75.5, 116.0)
save('bs_crescendo', loop(s, 1.0))
# remate corto para cuando el peligro se dispara
s, a, b = cut(102.0, 109.5)
st = s[:, :int(7.5 * SR)].copy()
st[:, -int(1.5 * SR):] *= np.linspace(1, 0, int(1.5 * SR))
save('bs_alerta', st)
