"""Reconstruye la banda sonora de una demo grabada con NAVE_DEMO.

Uso: python3 mezcla_demo.py <carpeta_demo> <carpeta_assets> <salida.wav>

Lee audio.log (lo escribe el juego): cada sonido con su instante, volumen, velocidad y paneo,
y los niveles de música, motor y ambientes en cada fotograma. Mezcla en estéreo a 48 kHz,
con un compresor suave y un limitador al final.
"""
import sys, os, numpy as np, soundfile as sf
from scipy import signal

DEMO, ASSETS, OUT = sys.argv[1:4]
SR = 48000
cache = {}


def load(rel):
    if rel not in cache:
        x, sr = sf.read(os.path.join(ASSETS, rel), always_2d=True)
        if sr != SR:
            x = signal.resample_poly(x, SR, sr, axis=0)
        if x.shape[1] == 1:
            x = np.repeat(x, 2, 1)
        cache[rel] = x.astype(np.float32)
    return cache[rel]


def stretch(x, speed):
    """Cambio de velocidad (y tono), como hace el motor de audio."""
    if abs(speed - 1.0) < 1e-3:
        return x
    n = int(len(x) / speed)
    idx = np.arange(n) * speed
    i0 = np.floor(idx).astype(int)
    f = (idx - i0)[:, None]
    i1 = np.minimum(i0 + 1, len(x) - 1)
    return x[i0] * (1 - f) + x[i1] * f


ones, loops, cuts, levels, end = [], {}, {}, [], None
for line in open(os.path.join(DEMO, 'audio.log')):
    p = line.split()
    if not p:
        continue
    if p[0] == 'S':
        ones.append((float(p[1]), p[2], p[3], float(p[4]), float(p[5]), float(p[6]), p[7] == '1'))
    elif p[0] == 'L':
        loops[p[2]] = (float(p[1]), p[3])
    elif p[0] == 'X':
        cuts.setdefault(p[2], float(p[1]))
    elif p[0] == 'M':
        levels.append([float(v) for v in p[1:]])
    elif p[0] == 'END':
        end = float(p[1])

levels = np.array(levels)
if end is None:
    end = levels[-1, 0]
N = int(end * SR) + SR
mix = np.zeros((N, 2), np.float32)


def env_at(col, smooth):
    """Nivel de un bucle por muestra (con el mismo suavizado exponencial que el juego)."""
    t = levels[:, 0]
    v = levels[:, col]
    tt = np.arange(N) / SR
    raw = np.interp(tt, t, v, left=v[0], right=v[-1])
    if smooth <= 0:
        return raw
    a = 1 - np.exp(-smooth / SR)
    return signal.lfilter([a], [1, -(1 - a)], raw, zi=[raw[0] * (1 - a)])[0]


tt = np.arange(N) / SR
# ---- música (tres pistas en bucle desde que arrancó el juego; sólo suena la elegida)
which = np.interp(tt, levels[:, 0], levels[:, 1])
lvl = np.interp(tt, levels[:, 0], levels[:, 2])
for k in range(3):
    key = f'music{k}'
    if key not in loops:
        continue
    t0, rel = loops[key]
    x = load(rel)
    pos = ((tt - t0) * SR).astype(np.int64) % len(x)
    target = np.where(np.round(which) == k, lvl, 0.0)
    a = 1 - np.exp(-1.8 / SR)
    g = signal.lfilter([a], [1, -(1 - a)], target)
    mix += x[pos] * g[:, None]

# ---- bucles con velocidad variable: motor y viento
def var_loop(rel, vol_col, speed_col, smooth=0.0, gain=1.0):
    x = load(rel)
    sp = np.interp(tt, levels[:, 0], levels[:, speed_col]) if speed_col else np.ones(N)
    ph = np.cumsum(sp) % len(x)
    i0 = ph.astype(np.int64)
    v = env_at(vol_col, smooth) * gain
    return x[i0] * v[:, None]


if 'engine' in loops:
    mix += var_loop(loops['engine'][1], 3, 4)
for k, (col, spcol, sm) in enumerate([(5, 0, 3.0), (6, 8, 3.0), (7, 0, 3.0)]):
    key = f'amb{k}'
    if key in loops:
        mix += var_loop(loops[key][1], col, spcol, sm)

# ---- música adaptativa (niveles ya suavizados por el juego)
for k in range(4):
    key = f'adapt{k}'
    if key in loops and levels.shape[1] > 9 + k:
        t0, rel = loops[key]
        x = load(rel)
        pos = ((tt - t0) * SR).astype(np.int64) % len(x)
        g = np.interp(tt, levels[:, 0], levels[:, 9 + k])
        mix += x[pos] * g[:, None]

# ---- sonidos sueltos
for (t, ent, rel, vol, speed, pan, spatial) in ones:
    if t < -5 or vol <= 0.0:
        continue
    try:
        x = stretch(load(rel), speed)
    except Exception as e:  # archivo faltante
        print('falta', rel, e)
        continue
    x = x * vol
    if spatial:
        # paneo como el motor espacial: el oído lejano baja hasta la mitad
        pan = float(np.clip(pan, -1, 1))
        gl = 0.75 * (1.0 - 0.5 * max(pan, 0.0)) + 0.25 * max(-pan, 0.0)
        gr = 0.75 * (1.0 - 0.5 * max(-pan, 0.0)) + 0.25 * max(pan, 0.0)
        x = x * np.array([gl, gr], np.float32)
    i = int(t * SR)
    if ent in cuts and cuts[ent] > t:
        n = int((cuts[ent] - t) * SR)
        x = x[:n].copy()
        f = min(len(x), int(0.04 * SR))
        if f:
            x[-f:] *= np.linspace(1, 0, f)[:, None]
    if i < 0:
        x = x[-i:]
        i = 0
    n = min(len(x), N - i)
    if n > 0:
        mix[i:i + n] += x[:n]

# ---- recorte al tramo grabado, compresor suave y limitador
mix = mix[: int(end * SR)]
env = np.abs(mix).max(1)
a, r = np.exp(-1 / (0.005 * SR)), np.exp(-1 / (0.25 * SR))
e = signal.lfilter([1 - r], [1, -r], env)
thr = 0.5
gain = np.where(e > thr, (thr + (e - thr) / 3.0) / np.maximum(e, 1e-9), 1.0)
mix = mix * gain[:, None]
mix = mix / (np.percentile(np.abs(mix), 99.9) + 1e-9) * 0.8
mix = np.tanh(mix * 1.1) / np.tanh(1.1) * 0.95
sf.write(OUT, mix.astype(np.float32), SR)
print(f'{OUT}: {len(mix) / SR:.1f} s, {len(ones)} sonidos, bucles {sorted(loops)}')
