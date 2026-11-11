"""Doblaje de radio en japonés (síntesis HTS con pyopenjtalk) + cadena de radio.
Voces: nitech m001 (BSD, masculina) y tohoku-f01 (CC-BY 4.0, femenina).
Uso: python3 voces.py <carpeta_wav> <archivo_rust_salida>
"""
import sys, os, numpy as np, soundfile as sf, pyopenjtalk
from pyopenjtalk.htsengine import HTSEngine
from scipy import signal

OUT, RS = sys.argv[1], sys.argv[2]
os.makedirs(OUT, exist_ok=True)
SR = 48000
F01 = '/tmp/claude-0/htsvoice-tohoku-f01/tohoku-f01-{}.htsvoice'
engines = {}


def eng(voice):
    if voice not in engines:
        if voice == 'm001':
            engines[voice] = HTSEngine(pyopenjtalk.DEFAULT_HTS_VOICE)
        else:
            engines[voice] = HTSEngine(F01.format(voice).encode())
    return engines[voice]


# personaje -> (voz, semitonos, velocidad, tipo de radio)
# personaje -> (voz, semitonos de tono, velocidad, tipo de radio, formante, intensidad)
# formante: semitonos de desplazamiento del timbre (negativo = voz más grande/grave).
# intensidad: 0 = calmado, 1 = combate, 2 = gritando.
CAST = {
    'Control': ('neutral', 0.0, 1.2, 'radio', 0.0, 0),
    'ControlU': ('angry', 0.5, 1.3, 'radio', 0.0, 1),
    'ControlF': ('happy', 0.0, 1.2, 'radio', 0.0, 0),
    'Alerta': ('neutral', -3.0, 1.2, 'sistema', 0.0, 0),
    # pilotos (voz masculina m001 transformada en cuatro personajes distintos)
    'Azul1': ('m001', -3.0, 1.25, 'radio', -3.0, 1),     # veterano, voz grande
    'Azul1U': ('m001', -1.5, 1.35, 'radio', -3.0, 2),
    'Azul2': ('m001', -1.0, 1.38, 'radio', -1.5, 2),     # novato
    'Azul3': ('m001', -5.0, 1.15, 'radio', -4.5, 1),     # sargento, muy grave
    'Azul4': ('m001', -2.0, 1.32, 'radio', -2.5, 2),     # temerario
    'Piloto': ('m001', -2.0, 1.32, 'casco', -2.0, 2),    # tú
    'Tierra': ('m001', -4.0, 1.2, 'radio', -3.5, 2),     # piloto de Destroid Spartan
}

# (prefijo exacto del subtítulo en español | id de grito, personaje, japonés)
LINES = [
    ("Escuadrón Orbital, aquí la nave nodriza", 'Control', "オービタル中隊へ、こちら母艦。インビッドの反応がセクターに侵入中。"),
    ("Intercepten y derriben ", 'Control', "敵を迎撃し、撃墜せよ。繰り返す、母艦に近づけるな。"),
    ("Azul 1 en tu ala", 'Azul1', "ブルーワン、隊長の僚機につく。行くぞ！"),
    ("¡Atención! Un enjambre Invid", 'ControlU', "警告！インビッドの大群が母艦に向かって直進中！"),
    ("Necesitamos ", 'Control', "主砲の充填まで、時間を稼いでください。持ちこたえて！"),
    ("Azul 3 y 4 cubriendo el casco", 'Azul3', "ブルースリー、フォー、船体を援護する。隊長、撃ち漏らしを頼む。"),
    ("Detectamos una colmena Invid", 'Control', "南東の砂漠にインビッドの巣を確認。偵察機はそこから出ています。"),
    ("Destrúyanla. Está marcada", 'Control', "巣を破壊してください。照準画面に、オレンジのひし形で表示しています。"),
    ("Consejo, líder: baja a menos", 'Azul1', "隊長、高度五百メートル以下で変形しろ。ガウォークかバトロイドなら、空中で止まって狙える。"),
    ("Inteligencia confirma plasma", 'Alerta', "誘導プラズマを確認。警報が鳴ったら、ブースト全開で急旋回せよ。"),
    ("Contacto prioritario", 'Control', "最優先目標。インビッドの指揮官機が、母艦に接近中。"),
    ("Si llega al casco", 'ControlU', "船体に到達されたら、母艦は終わりです。何としても撃墜して！"),
    ("Tiene escolta.", 'Azul1', "護衛付きだ。雑魚は俺たちが引き受ける。デカいのは任せたぞ、隊長。"),
    ("Es la hora.", 'Control', "時間です。インビッドの母艦は、低軌道にいます。"),
    ("Sube sobre los 3400", 'Control', "高度三千四百メートル以上に上昇し、軌道に出て撃破してください。"),
    ("Primero derriba sus torretas", 'Control', "まず砲台を破壊して。砲台が残っている限り、シールドが母艦を守っています。"),
    ("¡Contactos a las seis!", 'Azul1U', "六時方向に敵機！ブレイク、ブレイク！"),
    ("¡Interferencia Invid!", 'ControlU', "インビッドの妨害電波！レーダーが使えません、目視で戦って！"),
    ("¡Casco de la nodriza al 40%!", 'ControlU', "母艦の船体、四十パーセント！もう持ちません！"),
    ("¡Colmena a la vista!", 'Azul1U', "巣を目視で確認！偵察機を吐き出してるぞ、気をつけろ！"),
    ("¡La colmena pide refuerzos!", 'ControlU', "巣が増援を呼んでいます！後方に迎撃機！"),
    ("¡Un JEFE INVID emerge", 'Alerta', "警告。巣からインビッドの大型機が出現。偵察機とは桁違いの戦闘力です。"),
    ("El Comandante a 3 kilómetros", 'ControlU', "指揮官機、母艦まで三キロ！迎撃して！"),
    ("¡2 kilómetros!", 'ControlU', "二キロ！目の前です！"),
    ("¡UN KILÓMETRO!", 'ControlU', "一キロ！今すぐ撃墜して！"),
    ("El Comandante impactó", 'Alerta', "指揮官機が母艦に激突。作戦失敗。"),
    ("¡Comandante abatido!", 'ControlF', "指揮官機、撃墜！お見事です、隊長！"),
    ("Órbita alcanzada.", 'Control', "軌道に到達。前方にインビッド母艦。シールド展開中。"),
    ("Te sigo, líder.", 'Azul1', "ついて行くぜ、隊長。地球のために！"),
    ("¡Está perdiendo integridad!", 'ControlU', "敵母艦、損傷拡大中！攻撃を続けて！"),
    ("¡El escudo de la nave nodriza Invid", 'ControlU', "インビッド母艦のシールド消滅！船体を攻撃して！"),
    ("¡Nave nodriza Invid destruida!", 'ControlF', "インビッド母艦、撃沈！地球は救われました！"),
    ("Volvemos a casa, líder.", 'Azul1', "帰ろう、隊長。一緒に飛べて光栄だった。"),
    ("Quedan restos de la flota", 'Control', "敵艦隊の残存部隊あり。自由哨戒を許可します。"),
    ("¡Jefe Invid en el sector!", 'Alerta', "警告。セクターにインビッドの大型機を確認。"),
    ("Sector despejado.", 'ControlF', "セクター制圧完了。よくやったわ、隊長。"),
    ("¡Cañones cargados!", 'ControlF', "主砲、充填完了！撃て！敵の大群が撤退していきます。やりました！"),
    ("¡Colmena destruida!", 'ControlF', "巣を破壊！もう偵察機は出てきません！"),
    ("¡PLASMA EN COLA!", 'Alerta', "後方にプラズマ。ブーストで急旋回せよ。"),
    # gritos de combate (sólo voz)
    ("#p_misil", 'Piloto', "ミサイル、発射！"),
    ("#p_racimo", 'Piloto', "全弾発射！いっけえ！"),
    ("#p_derribo1", 'Piloto', "撃墜！"),
    ("#p_derribo2", 'Piloto', "一機、撃墜！"),
    ("#p_derribo3", 'Piloto', "よし、落とした！"),
    ("#p_jefe", 'Piloto', "大物を仕留めたぞ！"),
    ("#p_impacto1", 'Piloto', "くっ、被弾した！"),
    ("#p_impacto2", 'Piloto', "ぐあっ！"),
    ("#p_gerwalk", 'Piloto', "ガウォーク！"),
    ("#p_battroid", 'Piloto', "バトロイド、変形！"),
    ("#p_fighter", 'Piloto', "ファイターに戻す！"),
    ("#a1_derribo", 'Azul1', "ブルーワン、一機撃墜！"),
    ("#a3_derribo", 'Azul3', "ブルースリー、撃墜確認。"),
    ("#a1_racimo", 'Azul1U', "ブルーワン、ミサイル発射！"),
    ("#a1_aviso", 'Azul1U', "隊長、後ろにつかれてるぞ！"),
    ("#c_recarga", 'Control', "補給完了。全兵装、満タンです。"),
    # charla de combate de los pilotos (gritos con intensidad)
    ("#p_turbo", 'Piloto', "うおおおっ！振り切れえっ！"),
    ("#p_caer", 'Piloto', "落ちろおっ！"),
    ("#p_escapa", 'Piloto', "逃がすかよっ！"),
    ("#p_aguanta", 'Piloto', "まだだ、まだ終わらんっ！"),
    ("#a1_izq", 'Azul1U', "隊長、左から来るぞ！"),
    ("#a1_cubre", 'Azul1', "後ろは任せろ、隊長！"),
    ("#a2_engage", 'Azul2', "ブルーツー、交戦開始！"),
    ("#a2_rapido", 'Azul2', "ちくしょう、速すぎる！"),
    ("#a2_derribo", 'Azul2', "やった！一機落としたぞ！"),
    ("#a2_impacto", 'Azul2', "被弾した！まだ飛べる！"),
    ("#a3_cubre", 'Azul3', "落ち着け、ブルーツー。編隊を崩すな。"),
    ("#a3_racimo", 'Azul3', "ブルースリー、ミサイル全弾発射。"),
    ("#a4_engage", 'Azul4', "ブルーフォー、突っ込むぜえっ！"),
    ("#a4_derribo", 'Azul4', "どうだ、見たか！"),
    ("#a4_racimo", 'Azul4', "まとめて吹っ飛べえっ！"),
    # Soldado de Choque y frentes terrestres
    ("¡Un SOLDADO DE CHOQUE Invid", 'Alerta', "警告。インビッドのショックトルーパーが母艦に接近中。装甲が非常に厚い、注意せよ。"),
    ("¡Soldado de choque en el sector!", 'Alerta', "警告。セクターにショックトルーパーを確認。"),
    ("En tierra, los Destroid Spartan", 'Control', "地上では、デストロイド・スパルタンが四つのセクターを防衛中です。余裕があれば援護して。"),
    ("#s_fuego", 'Tierra', "スパルタン隊、対空射撃開始！"),
    ("#s_ayuda", 'Tierra', "こちら地上部隊、航空支援を要請する！"),
    ("#s_gracias", 'Tierra', "援護感謝する、バルキリー！"),
]


def tts(text, voice, semis, speed):
    labels = pyopenjtalk.extract_fullcontext(text)
    e = eng(voice)
    e.set_speed(speed)
    e.add_half_tone(semis)
    x = e.synthesize(labels)
    e.refresh()
    return x / 32768.0


rng = np.random.default_rng(7)


def bpf(x, lo, hi, o=4):
    sos = signal.butter(o, [lo / (SR / 2), hi / (SR / 2)], 'band', output='sos')
    return signal.sosfilt(sos, x)


def compress(x, thr=0.25, ratio=4.0):
    env = np.abs(x)
    a = np.exp(-1 / (0.003 * SR))
    r = np.exp(-1 / (0.08 * SR))
    e = 0.0
    out = np.empty_like(x)
    for i, v in enumerate(env):
        e = a * e + (1 - a) * v if v > e else r * e + (1 - r) * v
        g = 1.0 if e < thr else (thr + (e - thr) / ratio) / e
        out[i] = x[i] * g
    return out


def formant_shift(x, semis):
    """Desplaza el timbre: re-muestrea (cambia tono y formantes) y el tono ya viene compensado."""
    if abs(semis) < 0.05:
        return x
    r = 2 ** (semis / 12)
    up, down = int(round(1000 / r)), 1000
    return signal.resample_poly(x, up, down)


def intensify(x, level):
    """Voz forzada: excitador de armónicos, saturación y compresión fuerte."""
    if level <= 0:
        return x
    x = x / (np.abs(x).max() + 1e-9)
    hi = signal.sosfilt(signal.butter(2, 2400 / (SR / 2), 'high', output='sos'), x)
    drive = 3.0 + 2.5 * level
    exc = np.tanh(hi * drive) / np.tanh(drive)
    lo = signal.sosfilt(signal.butter(2, [90 / (SR / 2), 500 / (SR / 2)], 'band', output='sos'), x)
    y = x + exc * (0.12 + 0.06 * level) + lo * 0.45 * level
    y = compress(y / np.abs(y).max(), 0.18, 6.0 + 2 * level)
    return np.tanh(y / np.abs(y).max() * (1.5 + 0.8 * level)) / np.tanh(1.5 + 0.8 * level)


def radio_chain(x, kind):
    x = x / (np.abs(x).max() + 1e-9)
    x = np.concatenate([np.zeros(int(0.06 * SR)), x, np.zeros(int(0.12 * SR))])
    n = len(x)
    t = np.arange(n) / SR
    if kind == 'casco':  # micro del casco: más cuerpo, menos filtrado
        y = bpf(x, 110, 5000, 2)
        y = np.tanh(y * 2.2) / np.tanh(2.2)
        y = compress(y, 0.3, 3)
        y += bpf(rng.standard_normal(n), 300, 3000) * 0.012
        return y
    y = bpf(x, 330, 3300, 4)
    y = np.tanh(y * 3.5) / np.tanh(3.5)
    if kind == 'sistema':  # voz de sistema: modulación en anillo + peine metálico
        y = y * (0.65 + 0.35 * np.sin(2 * np.pi * 42 * t))
        d = int(0.0045 * SR)
        z = y.copy()
        for i in range(d, n):
            z[i] += 0.45 * z[i - d]
        y = z / np.abs(z).max()
    y = compress(y, 0.25, 4)
    # estática de fondo con crepitar
    st = bpf(rng.standard_normal(n), 900, 4200) * 0.035
    gate = (np.abs(signal.lfilter([1], [1, -0.999], rng.standard_normal(n) * 0.02)) > 0.6).astype(float)
    gate = signal.lfilter([0.002], [1, -0.998], gate)
    st *= 1 + 2.5 * gate
    y = bpf(y + st, 250, 3800, 2)
    # cola de squelch
    tail = int(0.12 * SR)
    y[-tail:] += bpf(rng.standard_normal(tail), 1500, 6000) * np.linspace(0.25, 0, tail)
    return y


rows = []
for key, who, jp in LINES:
    voice, semis, speed, kind, formant, level = CAST[who]
    r = 2 ** (formant / 12)
    # el re-muestreo cambia tono y duración: se compensa en la síntesis
    x = tts(jp, voice, semis - formant, speed / r)
    x = formant_shift(x, formant)
    x = intensify(x, level)
    y = radio_chain(x, kind)
    y = y / (np.abs(y).max() + 1e-9) * 0.89
    idx = len(rows)
    name = f'voz_{idx:02d}'
    sf.write(os.path.join(OUT, name + '.wav'), y.astype(np.float32), SR)
    rows.append((key, name, len(y) / SR, jp))
    print(name, who, f'{len(y) / SR:.2f}s', jp)

with open(RS, 'w') as f:
    f.write('//! Tabla generada por herramientas/voces.py: subtítulo (prefijo) o grito -> voz japonesa.\n')
    f.write('//! (prefijo del texto en español, archivo en assets/audio/voz/, duración en segundos, japonés)\n\n')
    f.write('pub const VOCES: &[(&str, &str, f32, &str)] = &[\n')
    for k, nme, d, jp in rows:
        f.write(f'    ({k!r}, "{nme}", {d:.2f}, "{jp}"),\n'.replace("'", '"'))
    f.write('];\n')
