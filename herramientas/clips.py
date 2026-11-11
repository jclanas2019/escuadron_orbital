"""Extrae capas de efectos (explosiones, enjambre de misiles, lanzamiento) de los dos clips
aportados, quitando la parte armónica (música) con HPSS."""
import sys, os, numpy as np, librosa, soundfile as sf
from scipy import signal

SR = 44100
skull, maxf, out = sys.argv[1], sys.argv[2], sys.argv[3]
os.makedirs(out, exist_ok=True)


def load(p):
    y, _ = librosa.load(p, sr=SR, mono=True)
    return y


def clean(seg, keep_harm=0.15):
    D = librosa.stft(seg, n_fft=2048, hop_length=512)
    H, P = librosa.decompose.hpss(D, margin=(1.0, 2.0))
    R = D - H - P
    y = librosa.istft(P + R * 0.8 + H * keep_harm, hop_length=512, length=len(seg))
    b, a = signal.butter(2, 30 / (SR / 2), 'high')
    return signal.lfilter(b, a, y)


def cut(y, t0, t1, fi=0.004, fo=0.25, keep_harm=0.15):
    s = y[int(t0 * SR):int(t1 * SR)].copy()
    s = clean(s, keep_harm)
    n = len(s)
    s[:int(fi * SR)] *= np.linspace(0, 1, int(fi * SR))
    f = int(fo * SR)
    s[-f:] *= np.linspace(1, 0, f) ** 2
    return s / (np.abs(s).max() + 1e-9) * 0.89


def onset_near(y, t, win=0.6):
    a = int(max(0, t - win / 2) * SR)
    seg = y[a:a + int(win * SR)]
    env = librosa.onset.onset_strength(y=seg, sr=SR, hop_length=256)
    return a / SR + np.argmax(env) * 256 / SR - 0.01


ys = load(skull)
ym = load(maxf)
items = [
    ('clip_explosion_1', ys, onset_near(ys, 38.5), 2.2),
    ('clip_explosion_2', ys, onset_near(ys, 33.6), 2.0),
    ('clip_explosion_3', ym, onset_near(ym, 38.2), 2.4),
    ('clip_impacto', ys, onset_near(ys, 47.4), 1.2),
    ('clip_enjambre', ys, 39.5, 2.4),
    ('clip_lanzamiento', ys, 22.25, 1.1),
]
for name, y, t0, d in items:
    s = cut(y, t0, t0 + d)
    sf.write(os.path.join(out, name + '.wav'), s.astype(np.float32), SR)
    print(name, round(t0, 2), d)
