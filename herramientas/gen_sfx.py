"""Genera las capas de efectos de alta calidad (44.1 kHz) para Escuadrón Orbital.
Uso: python3 gen_sfx.py <carpeta_salida_wav>
"""
import sys, os, numpy as np, soundfile as sf
from scipy import signal

SR = 44100
OUT = sys.argv[1]
os.makedirs(OUT, exist_ok=True)
rng = np.random.default_rng(1234)


def t_(d):
    return np.arange(int(d * SR)) / SR


def lp(x, f, o=2):
    b, a = signal.butter(o, min(f, SR / 2 - 100) / (SR / 2), 'low')
    return signal.lfilter(b, a, x)


def hp(x, f, o=2):
    b, a = signal.butter(o, f / (SR / 2), 'high')
    return signal.lfilter(b, a, x)


def bp(x, f1, f2, o=2):
    b, a = signal.butter(o, [f1 / (SR / 2), min(f2, SR / 2 - 100) / (SR / 2)], 'band')
    return signal.lfilter(b, a, x)


def sweep_lp(x, f0, f1, curve=2.0):
    """Paso bajo variable (filtro de un polo por muestra, 4 etapas)."""
    n = len(x)
    k = np.linspace(0, 1, n) ** (1 / curve)
    f = f0 * (f1 / f0) ** k
    a = 1 - np.exp(-2 * np.pi * f / SR)
    y = np.empty(n)
    s1 = s2 = s3 = s4 = 0.0
    for i in range(n):
        ai = a[i]
        s1 += (x[i] - s1) * ai
        s2 += (s1 - s2) * ai
        s3 += (s2 - s3) * ai
        s4 += (s3 - s4) * ai
        y[i] = s4
    return y


def pink(n):
    w = rng.standard_normal(n)
    b = [0.049922035, -0.095993537, 0.050612699, -0.004408786]
    a = [1, -2.494956002, 2.017265875, -0.522189400]
    return signal.lfilter(b, a, w) * 8


def brown(n):
    y = np.cumsum(rng.standard_normal(n))
    y = hp(y, 15)
    return y / (np.abs(y).max() + 1e-9)


def env_exp(n, attack, decay):
    t = np.arange(n) / SR
    return np.minimum(t / max(attack, 1e-4), 1.0) * np.exp(-t / decay)


def reverb_ir(dur, decay, bright=6000, stereo=False):
    n = int(dur * SR)
    t = np.arange(n) / SR
    chans = []
    for c in range(2 if stereo else 1):
        r = rng.standard_normal(n) * np.exp(-t / decay)
        r = lp(r, bright)
        # primeras reflexiones
        for d, g in [(0.013, 0.5), (0.029, 0.35), (0.047, 0.3), (0.071, 0.2)]:
            i = int((d + 0.004 * c) * SR)
            if i < n:
                r[i] += g * 4
        chans.append(r / np.sqrt((r ** 2).sum()))
    return chans


def add_reverb(x, wet=0.25, dur=2.5, decay=0.7, bright=5000):
    ir = reverb_ir(dur, decay, bright)[0]
    w = signal.fftconvolve(x, ir)[: len(x) + len(ir) - 1]
    y = np.zeros(len(w))
    y[: len(x)] += x
    return y * (1 - wet) + w * wet * 3


def stereo_reverb(x, wet=0.3, dur=2.5, decay=0.7, bright=5000):
    irs = reverb_ir(dur, decay, bright, stereo=True)
    outs = []
    for ir in irs:
        w = signal.fftconvolve(x, ir)
        y = np.zeros(len(w))
        y[: len(x)] += x
        outs.append(y * (1 - wet) + w * wet * 3)
    return np.stack(outs, 1)


def finish(x, peak=0.89, fade_out=0.05, trim_db=-46):
    x = np.asarray(x, dtype=np.float64)
    if x.ndim == 1:
        x = x - np.mean(x[: min(len(x), 2000)]) * 0
    # recortar silencio final
    mono = np.abs(x) if x.ndim == 1 else np.abs(x).max(1)
    thr = mono.max() * 10 ** (trim_db / 20)
    idx = np.where(mono > thr)[0]
    if len(idx):
        x = x[: idx[-1] + 1]
    n = len(x)
    f = min(int(fade_out * SR), n // 4)
    ramp = np.linspace(1, 0, f)
    if x.ndim == 1:
        x[-f:] *= ramp
    else:
        x[-f:] *= ramp[:, None]
    x = x / (np.abs(x).max() + 1e-9) * peak
    return x


def soft(x, drive=1.5):
    return np.tanh(x * drive) / np.tanh(drive)


def save(name, x):
    sf.write(os.path.join(OUT, name + '.wav'), finish(x).astype(np.float32), SR)


# ------------------------------------------------------------------ explosiones

def explosion(dur=3.5, size=1.0, seed=0, crack=1.0, sub_f=(95, 26), body_cut=(7000, 220), tail=2.6):
    global rng
    rng = np.random.default_rng(100 + seed)
    n = int(dur * SR)
    t = np.arange(n) / SR
    # 1) chasquido inicial (onda de choque)
    c = rng.standard_normal(n) * np.exp(-t / 0.006)
    c = hp(c, 900) * crack
    # 2) cuerpo: ruido rosa con filtro que se cierra
    body = pink(n)
    body = sweep_lp(body, body_cut[0], body_cut[1], 2.5) * env_exp(n, 0.004, 0.55 * size)
    body *= 1 + 0.35 * np.sin(2 * np.pi * 7 * t + rng.uniform(0, 6)) * np.exp(-t / 0.4)
    # 3) golpe de subgraves con barrido descendente
    f = sub_f[1] + (sub_f[0] - sub_f[1]) * np.exp(-t / 0.18)
    ph = 2 * np.pi * np.cumsum(f) / SR
    sub = np.sin(ph) * env_exp(n, 0.003, 0.45 * size)
    sub = soft(sub * 1.8, 1.2)
    # 4) retumbo largo
    rum = lp(brown(n), 140, 2) * env_exp(n, 0.05, tail * size * 0.5)
    # 5) crepitar de escombros
    crk = np.zeros(n)
    k = int(0.12 * SR)
    while k < n:
        a = np.exp(-(k / SR) / (0.9 * size))
        L = int(rng.uniform(0.002, 0.012) * SR)
        if k + L < n:
            crk[k:k + L] += rng.standard_normal(L) * np.hanning(L) * a * rng.uniform(0.3, 1)
        k += int(rng.exponential(0.035) * SR) + 1
    crk = bp(crk, 1200, 7000)
    x = c * 0.9 + body * 1.4 + sub * 1.1 + rum * 0.9 + crk * 0.45
    x = soft(x * 1.2, 1.6)
    x = add_reverb(x, wet=0.22, dur=2.8, decay=0.8 * size, bright=3500)
    return x


for i in range(3):
    save(f'explosion_cerca_{i + 1}', explosion(3.6, 1.0 + 0.15 * i, seed=i, sub_f=(110 - 10 * i, 28 + 2 * i)))


def far(x, cut=380, delay=0.0):
    y = lp(x, cut, 4)
    y = add_reverb(y, wet=0.5, dur=4.0, decay=1.4, bright=900)
    if delay:
        y = np.concatenate([np.zeros(int(delay * SR)), y])
    return y


for i in range(2):
    save(f'explosion_lejos_{i + 1}', far(explosion(4.5, 1.4, seed=10 + i, crack=0.3, tail=3.5), 420 - 120 * i))

# enorme: cadena de detonaciones
rng = np.random.default_rng(55)
n = int(7.5 * SR)
huge = np.zeros(n + SR * 6)
for j, (dt, s) in enumerate([(0, 1.6), (0.35, 1.0), (0.9, 1.3), (1.6, 0.9), (2.4, 1.2), (3.3, 0.8)]):
    e = explosion(4.5, s, seed=30 + j, sub_f=(80, 22), tail=4.0)
    i0 = int(dt * SR)
    huge[i0:i0 + len(e)] += e * (1.0 if j == 0 else 0.6)
huge = add_reverb(huge, wet=0.3, dur=4.5, decay=1.6, bright=2500)
save('explosion_enorme', huge)

# micro-explosiones (misiles en racimo)
for i in range(3):
    save(f'micro_explosion_{i + 1}', explosion(1.6, 0.45, seed=60 + i, crack=1.3, sub_f=(160, 60), body_cut=(9000, 600), tail=0.9))

# ------------------------------------------------------------------ misiles

def rocket(dur, seed, f_hi=6500, f_lo=1100, ign=True, recede=True):
    global rng
    rng = np.random.default_rng(200 + seed)
    n = int(dur * SR)
    t = np.arange(n) / SR
    hiss = pink(n) + rng.standard_normal(n) * 0.3
    # chisporroteo del motor cohete (modulación rápida)
    mod = 1 + 0.5 * lp(rng.standard_normal(n), 60) * 6
    hiss *= mod
    hiss = sweep_lp(hiss, f_hi, f_lo, 1.5) if recede else lp(hiss, f_hi)
    hiss = hp(hiss, 250)
    e = np.minimum(t / 0.02, 1) * (np.exp(-t / (dur * 0.45)) if recede else np.sin(np.pi * t / dur) ** 0.6)
    x = hiss * e
    if ign:
        pop = rng.standard_normal(n) * np.exp(-t / 0.015)
        pop = lp(pop, 2500) * 2.5
        thump = np.sin(2 * np.pi * np.cumsum(70 + 80 * np.exp(-t / 0.03)) / SR) * np.exp(-t / 0.07) * 1.5
        x = x + pop + thump
    return x


for i in range(2):
    save(f'misil_lanzamiento_{i + 1}', add_reverb(soft(rocket(1.9, i), 1.3), 0.2, 1.5, 0.5))

# paso de misil (doppler)
rng = np.random.default_rng(300)
n = int(1.4 * SR)
t = np.arange(n) / SR
x = pink(n) + rng.standard_normal(n) * 0.4
ctr = 0.55
fc = 900 + 4500 * np.exp(-((t - ctr) / 0.18) ** 2) + 2500 * (t < ctr)
y = np.zeros(n)
s = 0.0
for i in range(n):  # paso bajo con frecuencia variable (doppler aproximado)
    a = 1 - np.exp(-2 * np.pi * fc[i] / SR)
    s += (x[i] - s) * a
    y[i] = s
y = hp(y, 300) * np.exp(-((t - ctr) / 0.28) ** 2)
save('paso_misil', y)

# lanzamiento en racimo: 12 disparos rápidos + enjambre de silbidos (estéreo)
rng = np.random.default_rng(400)
D = 3.4
n = int(D * SR)
L = np.zeros(n)
R = np.zeros(n)
for k in range(12):
    st = int((0.02 + k * 0.055 + rng.uniform(-0.01, 0.01)) * SR)
    r = rocket(2.6, 40 + k, f_hi=rng.uniform(5000, 8000), f_lo=rng.uniform(700, 1400))
    r *= rng.uniform(0.5, 0.9)
    pan = rng.uniform(-0.9, 0.9)
    gl, gr = np.sqrt((1 - pan) / 2), np.sqrt((1 + pan) / 2)
    m = min(len(r), n - st)
    L[st:st + m] += r[:m] * gl
    R[st:st + m] += r[:m] * gr
# silbido agudo del enjambre (bandas desafinadas que se cruzan)
for k in range(5):
    f0 = rng.uniform(1800, 3200)
    f = f0 * (1 - 0.35 * t_(D)) + 120 * np.sin(2 * np.pi * rng.uniform(0.5, 1.5) * t_(D))
    tone = np.sin(2 * np.pi * np.cumsum(f) / SR) * 0.04 * np.exp(-t_(D) / 1.2) * np.minimum(t_(D) / 0.2, 1)
    p = rng.uniform(-1, 1)
    L += tone * (1 - p) / 2
    R += tone * (1 + p) / 2
st = np.stack([L, R], 1)
rv = stereo_reverb(st.mean(1), 0.25, 2.2, 0.6, 4000)
out = np.zeros_like(rv)
out[:n] += st * 0.8
out += rv * 0.35
save('racimo_lanzamiento', out)

# ------------------------------------------------------------------ radio

rng = np.random.default_rng(500)
n = int(0.22 * SR)
t = np.arange(n) / SR
click = np.zeros(n)
click[0:30] = np.hanning(30) * 1.0
kssh = bp(rng.standard_normal(n), 1500, 6000) * np.exp(-t / 0.05) * 0.6
beep = np.sin(2 * np.pi * 1850 * t) * ((t > 0.03) & (t < 0.11)) * 0.35
save('radio_abre', click + kssh + beep)
n = int(0.35 * SR)
t = np.arange(n) / SR
tail = bp(rng.standard_normal(n), 800, 5000) * np.exp(-t / 0.12) * 0.9
tail[:40] += np.hanning(40)
beep2 = np.sin(2 * np.pi * 1400 * t) * ((t > 0.0) & (t < 0.07)) * 0.3
save('radio_cierra', tail + beep2)


def loopable(x, fade=0.3):
    f = int(fade * SR)
    y = x[:-f].copy()
    w = np.linspace(0, 1, f)
    y[:f] = y[:f] * w + x[-f:] * (1 - w)
    return y


def save_loop(name, x, peak=0.7):
    x = x / (np.abs(x).max() + 1e-9) * peak
    sf.write(os.path.join(OUT, name + '.wav'), x.astype(np.float32), SR)


stat = bp(rng.standard_normal(SR * 4 + int(0.3 * SR)), 600, 4500)
stat *= 1 + 0.6 * lp(rng.standard_normal(len(stat)), 8) * 10
save_loop('radio_estatica', loopable(stat), 0.5)

# ------------------------------------------------------------------ ambientes en bucle

N = SR * 6 + int(0.4 * SR)
tt = np.arange(N) / SR
hum = (np.sin(2 * np.pi * 60 * tt) * 0.25 + np.sin(2 * np.pi * 120 * tt) * 0.12 + np.sin(2 * np.pi * 397 * tt) * 0.02)
air = lp(pink(N), 1800) * 0.25 + lp(brown(N), 90) * 0.8
cab = hum + air + bp(rng.standard_normal(N), 3000, 9000) * 0.015
save_loop('cabina_ambiente', loopable(cab, 0.4), 0.6)

w = pink(N)
g = 0.6 + 0.4 * np.sin(2 * np.pi * 0.17 * tt) * np.sin(2 * np.pi * 0.41 * tt + 1)
wind = sweep_lp(w, 900, 900) * g + bp(rng.standard_normal(N), 400, 1600) * 0.3 * g ** 2
save_loop('viento', loopable(wind, 0.4), 0.6)

# ------------------------------------------------------------------ armas e impactos

rng = np.random.default_rng(700)
n = int(0.16 * SR)
t = np.arange(n) / SR
thump = np.sin(2 * np.pi * np.cumsum(180 * np.exp(-t / 0.02) + 55) / SR) * np.exp(-t / 0.035)
snap = hp(rng.standard_normal(n), 3000) * np.exp(-t / 0.004) * 0.5
save('laser_capa', add_reverb(thump + snap, 0.15, 0.6, 0.15))

for i in range(2):
    n = int(0.9 * SR)
    t = np.arange(n) / SR
    modes = [(523, 0.18), (1187, 0.12), (1931, 0.08), (2857, 0.05), (3780, 0.04), (811, 0.14)]
    x = sum(np.sin(2 * np.pi * f * (1 + 0.04 * i) * t + rng.uniform(0, 6)) * np.exp(-t / d) for f, d in modes) * 0.3
    x += hp(rng.standard_normal(n), 1200) * np.exp(-t / 0.015)
    x += np.sin(2 * np.pi * np.cumsum(140 * np.exp(-t / 0.03) + 60) / SR) * np.exp(-t / 0.08) * 1.2
    save(f'impacto_metal_{i + 1}', add_reverb(soft(x, 1.5), 0.15, 0.8, 0.2))

# transformación: servos + golpes mecánicos
n = int(1.6 * SR)
t = np.arange(n) / SR
f = 220 + 380 * np.sin(np.pi * np.minimum(t / 1.1, 1)) ** 1.5
servo = signal.sawtooth(2 * np.pi * np.cumsum(f) / SR) * 0.18 + signal.sawtooth(2 * np.pi * np.cumsum(f * 1.503) / SR) * 0.1
servo = bp(servo, 200, 3500) * np.minimum(t / 0.05, 1) * np.clip((1.25 - t) / 0.2, 0, 1)
clunks = np.zeros(n)
for ct in [0.12, 0.38, 0.61, 0.83, 1.15]:
    i = int(ct * SR)
    L = int(0.25 * SR)
    tl = np.arange(L) / SR
    k = (np.sin(2 * np.pi * 95 * tl) * np.exp(-tl / 0.05) * 1.2 + hp(rng.standard_normal(L), 2000) * np.exp(-tl / 0.006) * 0.6
         + np.sin(2 * np.pi * 1730 * tl) * np.exp(-tl / 0.06) * 0.15)
    m = min(L, n - i)
    clunks[i:i + m] += k[:m]
save('transformacion', add_reverb(servo + clunks, 0.18, 1.0, 0.25))

print('ok', sorted(os.listdir(OUT)))
