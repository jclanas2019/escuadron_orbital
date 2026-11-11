# ESCUADRÓN ORBITAL

Combate de cazas en la Tierra y en la órbita con estética de **animación cel de los 90**
(contornos de tinta, sombras violeta, atardeceres rosados, HUD vectorial verde en
perspectiva, cabina con pantallas de colores y franjas de peligro). Hecho con **Bevy 0.20**
en Rust. El mundo (terreno, cielo, cabina, enemigos, pantallas y sonido) se genera por
código; el caza y el piloto son tus modelos 3D en `assets/models/`.

## Transformaciones del Valkyrie (tecla G)

| Forma | Dónde | Cómo vuela |
|---|---|---|
| **CAZA** | siempre | rápido (hasta 600 m/s con turbo), no puede detenerse |
| **GERWALK** | atmósfera, a menos de 500 m del suelo | hasta 190 m/s; puede quedarse suspendido (acelerador a 0), desplazarse de lado con A/D y posarse en el suelo; recibe 15 % menos de daño |
| **BATTROID** (robot) | cerca del suelo **y** en el espacio | lento (130 m/s) pero gira muy rápido, se desplaza de lado más rápido y recibe 35 % menos de daño |

* **G** pasa a la siguiente forma disponible. La transformación dura ~1 s, con humo y un
  corte de cámara lateral para verla.
* En el cielo (atmósfera alta) sólo se puede volar como CAZA: si subes transformado por
  encima de ~650 m del suelo, la nave se reconvierte sola.
* En GERWALK y BATTROID disparas con el fusil (más cadencia y menos calor que los láseres
  del caza). Los misiles funcionan en todas las formas.
* La forma actual se ve arriba a la izquierda (FORMA) y en la pantalla de estado (MODE).

## Frentes terrestres: Destroid Spartan

En la atmósfera hay **cuatro sectores** del desierto (OESTE, NORTE, COLMENA y SUR) con tres
**Destroid Spartan** cada uno. Grupos de Invid de asalto bajan en picado sobre ellos; los
Spartan giran el cuerpo hacia el blanco, disparan ráfagas antiaéreas y salvas de seis
micromisiles, y piden apoyo por radio cuando están en apuros («こちら地上部隊、航空支援を
要請する！»). Si cae uno, llega un refuerzo a los 35 s. Es una batalla de fondo: puedes
ayudar (los Invid que derribes cerca cuentan), pero no cambia los objetivos de la misión ni
retrasa las oleadas.

## Combate aéreo estilo Top Gun

Los Invid ya no vuelan en línea recta cuando los persigues: si te pones a su cola, un misil
los busca o se cruzan de frente, hacen **maniobras evasivas**: viraje de ruptura (break),
tonel, **Split-S** (medio tonel y picado invirtiendo el rumbo), **Immelmann** (medio rizo
hacia arriba) o zigzag. En dificultad media reaccionan antes y con más frecuencia. El
Escuadrón Azul hace lo mismo cuando un Invid se les pega a la cola.

## Nuevo jefe: Soldado de Choque Invid

Más grande y blindado que el Jefe Invid (950 de blindaje, 1400 en media). Además del plasma
guiado lanza una **lluvia de plasma** en abanico desde los hombros y hace **embestidas** a
560 m/s con un aura roja (aviso en pantalla: rompe el rumbo). Aparece en la misión 2 cuando
queda menos de la mitad del tiempo, directo a la nave nodriza, y alterna con el Jefe en la
patrulla libre. Chocar con un jefe ya no lo destruye: te empuja y te hace mucho daño.
Derribarlo da 2500 puntos.

## Misiles en racimo (circo Itano)

* **F / clic derecho**: misil directo (como antes, 8 por carga, se fija en el blanco).
* **V / botón central del ratón**: **salva en racimo** de 12 micromisiles. Salen en abanico
  hacia los lados, arriba y abajo, y luego se curvan en espiral hacia los Invid (el blanco
  fijado recibe más). Dejan estelas blancas continuas con contorno de tinta, como en el anime.
  Cada micromisil hace 40 de daño. Tienes **3 salvas**: se recupera una cada 20 s y se
  recargan todas en el anillo verde o al completar una misión (`R3` en la pantalla de estado,
  tres rombos en el HUD).
* Con los cortes automáticos (X) la cámara pasa a persecución en cámara lenta para ver el
  abanico de misiles.
* AZUL 1 a 4 también disparan salvas de 6 micromisiles de vez en cuando.

## Explosiones

Cada explosión se construye por capas: destello blanco, luz que ilumina las naves cercanas,
rayos en estrella, bolas de fuego amarillas, naranjas y rojas, onda de choque, humo claro y
humo oscuro con contorno de tinta, escombros y brasas que caen. Las grandes (jefe, colmena,
naves nodriza) encadenan detonaciones secundarias y un segundo anillo horizontal. Las cercanas
sacuden la cámara.

## Música adaptativa (Battle Stations)

*Battle Stations* (Robotech) se suma en capas a la música orquestal y reacciona al combate:

* **Suspenso**: cuando hay contactos acercándose en el radar (o durante el briefing), entra
  la sección tensa de la pista y la orquesta baja.
* **Batalla**: con los Invid encima suena la sección principal, separada en armonía y
  percusión que van en sincronía.
* **Crescendo**: el peligro (jefes cerca, casco bajo, plasma en cola, nodriza en apuros,
  alguien a tu cola, muchos Invid) sube la percusión y, si es alto, entra la sección de
  crescendo; al cruzar el umbral suena un golpe orquestal.
* Todo con fundidos: sube rápido y baja despacio. **M** sigue silenciando la música.

Las capas están en `assets/audio/adaptativa/` y se generan con
`herramientas/musica_adaptativa.py pista.mp3 carpeta`.

## Audio cinematográfico

Se conservan todos los sonidos de antes (`assets/audio/sfx/`) y encima se suman capas nuevas
de 44,1 kHz (`assets/audio/capas/`):

* **Explosiones** cercanas (golpe de subgraves, cuerpo, crepitar de escombros y reverberación),
  lejanas (retumbo filtrado) y enormes (cadena de detonaciones). En la atmósfera el sonido de
  una explosión lejana llega con retraso, como un trueno.
* **Sonido posicional**: explosiones, lanzamientos y pasadas de misiles suenan a la izquierda
  o a la derecha según dónde ocurren respecto a la cámara, y bajan de volumen con la distancia.
* **Misiles**: encendido y silbido del cohete, salva en racimo en estéreo y pasadas cerca de
  la cámara.
* Impactos metálicos al recibir daño, servos y golpes mecánicos al transformarse, refuerzo
  grave de los láseres.
* **Ambientes en bucle**: zumbido de cabina en las vistas interiores, viento según la
  velocidad en la atmósfera y estática de radio bajo las voces.
* Capas extraídas de los clips *Macross Skull Squadron* y *Max vs Milia*, con la música
  quitada: `clip_explosion_*`, `clip_impacto`, `clip_enjambre`, `clip_lanzamiento`.

**Pilotos con voz masculina e intensidad**: los cuatro pilotos del Escuadrón Azul y tú
tienen voces distintas (veterano de voz grande, novato nervioso, sargento grave, temerario
y tú, un as de sangre caliente), con la voz forzada como en pleno combate (armónicos,
saturación y compresión). Gritan al encender el turbo («うおおおっ！振り切れえっ！»), al
fijar un blanco («落ちろおっ！»), al recibir daño, al lanzar salvas y derribar Invid, y cada
cierto tiempo hay charla de combate entre ellos si hay enemigos cerca.

**Radio en japonés**: todos los mensajes de radio están doblados al japonés (CONTROL con voz
femenina, AZUL 1 y AZUL 3 con voces masculinas y ALERTA con voz de sistema), con filtro de
radio, chasquido de apertura y cierre, y los subtítulos en español abajo. La música baja un
poco mientras hablan. Además hay gritos de combate en japonés: al lanzar misiles («ミサイル、
発射！»), salvas en racimo («全弾発射！»), derribos, impactos, transformaciones («ガウォーク！»,
«バトロイド、変形！») y recarga.

`demo_audio_combate.ogg` es una mezcla de muestra de 24 s con estas capas.

Herramientas en `herramientas/`: `gen_sfx.py` (síntesis de las capas), `clips.py`
(extracción de los clips) y `voces.py` (doblaje, genera `src/voces.rs`).

## Jefe Invid y Colmena

* **Jefe Invid** (~24 m): 600 de blindaje (900 en media), láseres que hacen 80 % más
  daño y ráfagas de plasma guiado (2 en básica, 3 en media). Es el Comandante de la
  misión 4, emerge de la colmena en la misión 3 y aparece de vez en cuando en la patrulla
  libre. Derribarlo da 1500 puntos.
* **Colmena Invid** (~500 m): base terrestre al sureste del mapa. Mientras exista lanza
  exploradores cuando te acercas. Destruirla da 3000 puntos.

## Campaña y dificultad

En la pantalla de título elige la dificultad con las flechas **← →**:

| | BÁSICA | MEDIA |
|---|---|---|
| Daño de los Invid | 60 % | 100 % |
| Cadencia de disparo enemiga | más lenta | normal |
| Blindaje y velocidad Invid | 75 % / 85 % | 100 % |
| Daño a la nave nodriza | 50 % | 100 % |
| Recarga del escudo | rápida | normal |
| Fijar blanco | 0,5 s | 0,8 s |
| Plasma guiado Invid | no | sí (desde la misión 3) |

**Cinco misiones** con mensajes de radio de CONTROL y del Escuadrón Azul (subtítulos
abajo), objetivo actual arriba a la derecha y un cartel al empezar y terminar cada una:

1. **Primer contacto**: derriba 5 Invid (8 en media). A mitad de camino, emboscada por la cola.
2. **Defender la nodriza**: resiste 70 s (90 s en media) mientras grupos Invid atacan
   la nave nodriza. A mitad de tiempo, interferencia: el radar y la cámara de puntería
   quedan en estática y las retículas parpadean. Pitidos en los últimos 10 s.
3. **La colmena**: destruye la colmena Invid (rombo naranja). Al 75 % pide refuerzos y al
   50 % **emerge un Jefe Invid**. Consejo: baja y transfórmate en GERWALK o BATTROID para
   quedarte suspendido y concentrar el fuego. En media aparece además el **plasma guiado**: una alarma que se acelera al acercarse. Escápale con turbo y viraje
   cerrado, o derríbalo con láseres.
4. **El Comandante**: un Jefe Invid de élite con aura roja (rombo rojo con «!») vuela directo a la nave
   nodriza con escolta. La radio cuenta la distancia: 3 km… 2 km… 1 km. Si llega, la
   misión falla y la nodriza pierde un 30 % de casco.
5. **Asalto orbital**: sube a la órbita. La nave nodriza Invid tiene un **escudo** rosado
   que la hace invulnerable mientras queden sus torretas. Destrúyelas, rompe el escudo y
   acaba con ella. Al ganar sigue la **patrulla libre** con oleadas infinitas.

Si una misión falla, se reintenta sola. Si te derriban o cae la nave nodriza, pulsa
**R** para repetir la misión actual. Al completar una misión recibes misiles, escudo y
reparación.

## Misión

* **Nave nodriza terrestre**: los Invid la atacan; se defiende con su artillería. Si cae,
  pierdes (R para reintentar).
* **Escuadrón Azul**: cuatro Valkyries aliados. AZUL 1 y 2 son tus escoltas: vuelan en
  formación a tus costados y atacan a los Invid que se te acercan. AZUL 3 y 4 patrullan
  alrededor de la nave nodriza terrestre y cazan a los Invid que la atacan. Los Invid también les disparan; si derriban a uno, despega un
  reemplazo desde la nave nodriza cada 15 s. En el HUD llevan un chevrón azul con su
  número y en el radar son puntos celestes.
* **Punto de recarga**: el anillo verde sobre la popa de la nave nodriza terrestre.
  Atraviésalo para recargar 8 misiles, escudo al 100 %, reparar el casco y enfriar los
  láseres. Luego se pone rojo unos segundos. Un rombo verde en el HUD y un punto verde en
  el radar indican dónde está; la luz REARM del panel se enciende cuando está listo.

## Modelos 3D (optimizados)

| Archivo | Uso | Original → optimizado |
|---|---|---|
| `valkyrie_flight.glb` | tu caza en vuelo (cabina cerrada) | 275k → 82k vértices |
| `valkyrie.glb` | caza aparcado en la escena de título | 596k → 96k |
| `valkyrie_gerwalk.glb` / `valkyrie_battroid.glb` | tus formas GERWALK y BATTROID | 20k / 17k vértices, texturas 4096 → 2048 |
| `invid_boss.glb` | Jefe Invid | 22k vértices, texturas 4096 → 1024 |
| `invid_shock.glb` | Soldado de Choque Invid | 13k vértices, texturas 4096 → 1024 |
| `destroid_spartan.glb` | Destroid Spartan (NPC terrestres) | 2,4M → 60k triángulos, 100 MB → 2,8 MB |
| `invid_hive.glb` | Colmena Invid | 1,7M → 142k vértices, 126 MB → 9 MB |
| `valkyrie_blue.glb` | Escuadrón Azul (aliados NPC) | 194k → 33k, texturas 1024 |
| `pilot.glb` | piloto con esqueleto y animaciones | 17k (sin cambios de malla) |
| `invid.glb` / `invid_parts.glb` | cazas Invid / piezas al destruirlos | 216k → 38k / 255k → 44k |
| `invid_mothership.glb` | nave nodriza Invid (~880 m) | 1.9M → 182k |
| `carrier.glb` | nave nodriza terrestre (~630 m) | 548k → 105k |

Las texturas se redujeron de 4096 a 2048 px (1024 en las piezas) y se recomprimieron.
Total: de ~360 MB a ~48 MB. Además, los Invid a más de 1,6 km se dibujan con una silueta
simple (LOD), así que una oleada grande no pesa en la GPU.

Para cambiar un modelo basta con reemplazar su archivo en `assets/models/` (mismo nombre).

## Audio (OGG)

Todo el sonido está en `assets/audio/` en formato **Ogg Vorbis** y se puede reemplazar
por otro archivo con el mismo nombre (si falta alguno, el juego lo sintetiza por código).

Música: *Macross OST – Epic Cinematic Orchestral Reimagined* (aportada por el usuario
como música libre de regalías; ver nota de licencia abajo).

* `musica/musica_completa.ogg`: la pista completa (4:28), música de combate.
* `musica/musica_titulo.ogg`: el pasaje tranquilo (0:53–1:20) para la escena de título.
* `musica/musica_climax.ogg`: el clímax orquestal (2:57–final), suena en la misión del
  Comandante y en el asalto orbital.
  La música cambia sola entre las tres con fundidos; **M** la silencia.
* `sfx/`: efectos sintetizados por el juego (sin material de terceros): `laser`,
  `laser_enemigo`, `antiaereo`, `misil`, `fijado`, `alarma`, `impacto`, `explosion`,
  `explosion_grande`, `motor`, `golpe_bombo`, `platillo` y `transicion`.
  Para regenerarlos: `nave_combate --exportar-sonidos carpeta`.

`separar_audio_ia.sh cancion.mp3` separa cualquier pista en batería, bajo, voces y otros
con IA (Demucs), por si quieres pistas sueltas.

**Voces japonesas:** síntesis con Open JTalk / HTS. Voz femenina *tohoku-f01*
© 2015 Intelligent Communication Network (Ito-Nose) Laboratory, Tohoku University,
licencia **CC BY 4.0** (ver `assets/audio/voz/LICENCIA-voz-tohoku-f01.txt`). Voz masculina
*nitech_jp_atr503_m001* (licencia BSD modificada, Nagoya Institute of Technology).

**Battle Stations (Robotech):** pista aportada por el usuario; tiene derechos de autor.
Úsala para uso personal y bórrala de `assets/audio/adaptativa/` antes de publicar el juego
(sin esos archivos el juego simplemente no usa la música adaptativa).

**Clips de Macross:** las capas `clip_*` vienen de los clips aportados por el usuario y pueden
tener derechos de autor. Sirven para uso personal; antes de publicar el juego bórralas (el
juego funciona igual sin ellas, con las capas sintetizadas).

**Licencia:** se eliminó del juego todo el material de la canción anterior. Antes de
publicar el juego, confirma la licencia de esta pista orquestal en su fuente original:
al ser una reinterpretación del OST de Macross, la composición puede seguir teniendo
derechos aunque la grabación sea libre.

## Jugar

```bash
./start.sh
```

* En **Linux x86_64** arranca el binario precompilado incluido (`bin/linux-x86_64/`).
* En otros sistemas (macOS, Linux ARM…) instala Rust si falta, las librerías del sistema
  que necesita Bevy y compila el juego (la primera compilación tarda varios minutos).
* `./start.sh --build` fuerza la compilación desde el código fuente.

**Windows**: doble clic en **`start.bat`**. La primera vez instala Rust si falta (con la
cadena MSVC si tienes Visual Studio Build Tools, o la GNU, que no necesita nada más) y compila
el juego; las siguientes veces arranca directamente. `start.bat --build` fuerza recompilar.

## Controles

| Tecla | Acción |
|---|---|
| Ratón | cabeceo / guiñada |
| A / D | alabeo (rolar) |
| Q / E, flechas | guiñada / cabeceo con teclado |
| W / S | acelerador · **SHIFT** turbo |
| Clic izquierdo / Espacio | láseres |
| Clic derecho / F | misil directo (mantén al enemigo en la retícula hasta que se fije) |
| **V** / botón central | **misiles en racimo** (12 micromisiles, 3 salvas) |
| **G** | transformar: CAZA / GERWALK / BATTROID |
| **C** | siguiente cámara · **1–6** elegir cámara |
| **X** | cortes cinematográficos automáticos sí/no |
| I | invertir eje Y · H ayuda · Esc pausa · R reiniciar tras ser derribado |
| M | silenciar / activar la música |

## Mando de juego (Nintendo, PS5, Xbox)

Conecta un mando por USB o Bluetooth y el juego lo detecta solo. Los nombres de los botones
en pantalla (ayuda con **H**) siguen el **diseño** del mando: se elige en `assets/mando.cfg`
(`diseño = auto | nintendo | ps5 | xbox`) o con **F8** en el juego. En `auto` se reconoce
por el fabricante (Nintendo Switch Pro / Joy-Con, DualSense / DualShock, Xbox).

| Acción | PS5 (DualSense) | Nintendo (Switch Pro) | Xbox |
|---|---|---|---|
| Cabeceo / alabeo | stick izquierdo | stick izquierdo | stick izquierdo |
| Guiñada | stick derecho ← → | stick derecho ← → | stick derecho ← → |
| Acelerar / frenar (analógico) | R2 / L2 | ZR / ZL | RT / LT |
| Turbo | R1 | R | RB |
| Láseres | ✕ | B | A |
| Misil directo | ○ | A | B |
| Misiles en racimo | □ | Y | X |
| Transformar | △ | X | Y |
| Cámara | L1 · cruceta → | L · cruceta → | LB · cruceta → |
| Cortes cinematográficos | R3 · cruceta ← | stick der pulsado · cruceta ← | RS · cruceta ← |
| Invertir eje Y | L3 | stick izq pulsado | LS |
| Ayuda / música | CREATE · cruceta ↑ / cruceta ↓ | − · cruceta ↑ / cruceta ↓ | VIEW · cruceta ↑ / cruceta ↓ |
| Pausa · empezar · volver a despegar | OPTIONS | + | MENU |

* La asignación es **por posición** (el botón de abajo siempre dispara). Todo se puede
  cambiar en `assets/mando.cfg`, con el mismo formato que tus otros juegos.
* Algunos controladores de Nintendo informan los botones por letra en vez de posición; si en
  tu mando A y B salen al revés, pon `intercambiar_ab = sí`.
* **El mando no actúa hasta que pulsas un botón en él.** Al conectarlo se mide durante
  medio segundo la posición de reposo de cada stick y gatillo (así un stick descentrado o un
  gatillo que reposa en −1 no mueven la nave), y se ignoran los sensores de movimiento del
  Switch Pro y los Joy-Con, que el sistema muestra como otro "mando" que no para de moverse.
* **F7: asistente de configuración.** Te pide cada control (disparar, misil, racimo,
  transformar, turbo, cámara, pausa, los dos sticks y los gatillos), mide lo que pulsas o
  mueves y guarda un **perfil de ese mando** en `assets/mandos/`. Se carga solo cada vez que
  lo conectas. Sirve para mandos genéricos o clones que envían botones no estándar.
  Retroceso salta un paso; F7 lo cancela.
* **F6** activa la prueba del mando: cada botón o eje que muevas muestra su nombre.
* Los ejes que se mueven solos durante la calibración (el giroscopio de los mandos en modo
  Switch) se ignoran automáticamente.

### macOS: mandos por SDL2

En el Mac, el lector de mandos que trae Bevy no sabe hablar con el **mando Pro de Switch** ni
con los mandos chinos "compatibles con Nintendo": por Bluetooth no reaccionan y por cable mandan
datos sin sentido. Por eso, en macOS `start.sh` instala **SDL2** con Homebrew
(`brew install sdl2`) y compila el juego con `--features sdl`: SDL2 tiene controladores para
Switch Pro, Joy-Con, Xbox y DualSense, separa el giroscopio y entrega los botones por posición.
Si no tienes Homebrew, instálalo desde https://brew.sh y vuelve a ejecutar `./start.sh --build`.
(`NAVE_SIN_SDL=1 ./start.sh --build` compila sin SDL2.)

### Thunderobot G50S (y otros mandos chinos "compatibles con Nintendo")

* **En Mac, por Bluetooth**, cualquiera de estos dos modos funciona con SDL2:
  * **Modo S** (interruptor trasero en **S**, mantén **HOME** ~3 s): se empareja como
    *Pro Controller* (el que ya tienes conectado).
  * **Modo X** (interruptor en **X**, mantén **HOME** ~3 s hasta que parpadeen LED2+LED3): se
    empareja como *Xbox Wireless Controller*.
* **Por cable USB en Mac no**: arranca en XInput, que macOS no reconoce, y es lo que
  "vuelve loca" la cámara. Usa Bluetooth. En Windows funciona por cable o con el receptor 2.4G.
* Al conectarlo verás «Mando detectado…»: **pulsa cualquier botón** para activarlo.
* Si los sticks derivan: calíbralo (encender con **BACK + HOME**, dejarlo plano, START, apretar
  LT y RT a fondo 2-3 veces, girar los dos sticks 2-3 vueltas, START). Si dispara solo, quita
  el turbo: **MODE + TURBO** 3 s.
* Para ver los nombres al estilo Nintendo en pantalla: **F8** o `diseño = nintendo`.

## Grabar una demo cinemática

```bash
NAVE_DEMO=demo ./bin/linux-x86_64/nave_combate        # o: NAVE_DEMO=demo cargo run --release
bash herramientas/montar_demo.sh demo escuadron_orbital_demo.mp4
```

El juego se graba solo siguiendo un guion (título, despegue, combate con salvas en racimo,
Jefe Invid, subida a la órbita y gran explosión). El tiempo avanza en pasos fijos de 1/24 s,
así el video sale fluido aunque el equipo dibuje lento, y cada sonido queda anotado en
`demo/audio.log`. `montar_demo.sh` reconstruye la banda sonora exacta, une imagen y sonido
en 1080p y añade fundidos y el cartel final. El guion está en `src/demo.rs` (`GUION`).

## Escena de título

Al iniciar verás al piloto caminando junto al Valkyrie en la pista al atardecer. Haz clic o
pulsa ENTER: el piloto corre hacia su nave, fundido a negro y despegas.

## Panel de control

* **Pantalla central**: anunciadores (MASTER, LOCK, HEAT, SHLD, MSL, ALT, BOOST, AUTO),
  acelerador en pirámide, velocidad, horizonte artificial y radar con barrido.
* **Pantalla izquierda – cámara de puntería**: video en vivo y con zoom del enemigo fijado
  (o del más cercano), con distancia, tipo y vida del blanco.
* **Pantalla derecha – estado**: silueta de la nave coloreada por daño, anillo de escudo,
  misiles, calor del láser, zona y derribos.
* Relojes analógicos con aguja (velocidad y calor), botón de precaución maestra que
  parpadea con daño, botones de borde, tornillos e interruptores.

## Cámaras

1. **Ojos del piloto**: la cabeza se inclina con las fuerzas G, consola y manos en los mandos.
2. **Sobre el hombro**: casco, consola, palanca y el HUD proyectado en perspectiva.
3. **Cámara al piloto**: desde el panel, mirando al casco con el cielo detrás.
4. **Persecución**: detrás del caza, con inercia en los giros.
5. **Lateral**: perfil del caza acompañándolo.
6. **Paso de cámara**: cámara fija que ve pasar el caza (teleobjetivo).

Con los **cortes automáticos** (X) el director hace un primer plano del Invid al fijarlo,
una cámara de derribo que sigue sus piezas al destruirlo, y además corta a un primer plano de la mano en la
palanca al disparar un misil, a la cara del piloto al recibir un impacto fuerte, a un paso
de cámara al entrar en una zona, y hace cámara lenta en algunos derribos.

## Zonas

* **Atmósfera**: desierto al atardecer, ciudad iluminada, bases con torretas antiaéreas,
  oleadas de Invid y la nave nodriza terrestre sobre las nubes (2650 m). Sube sobre
  3400 m para salir a la órbita.
* **Órbita**: la Tierra, nebulosa, galaxia espiral, asteroides, la nave nodriza terrestre
  y la nave nodriza Invid. Baja hacia la Tierra para la reentrada.

La fuente DejaVu Sans Mono va incrustada (licencia libre, ver `fonts/LICENSE-DejaVu.txt`).
