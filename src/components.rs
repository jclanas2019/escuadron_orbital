use bevy::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Zone {
    Atmosphere,
    Space,
}

#[derive(Resource)]
pub struct Game {
    pub started: bool,
    pub paused: bool,
    pub zone: Zone,
    pub pending_zone: Option<Zone>,
    pub reset_on_switch: bool,
    pub fade: f32,
    pub fade_dir: f32,
    pub score: u32,
    pub wave: u32,
    pub wave_timer: f32,
    pub hull: f32,
    pub shield: f32,
    pub shield_delay: f32,
    pub missiles: u32,
    pub missile_reload: f32,
    pub missile_cd: f32,
    /// Salvas de micromisiles en racimo disponibles (máx. 3).
    pub cluster: u32,
    pub cluster_reload: f32,
    pub dead: bool,
    pub msg: String,
    pub msg_timer: f32,
    pub shake: f32,
    pub hit_flash: f32,
    pub gun_cd: f32,
    pub gun_side: bool,
    pub heat: f32,
    pub overheated: bool,
    pub lock: Option<Entity>,
    pub lock_time: f32,
    pub locked: bool,
    pub show_help: bool,
    pub mothership_alive: bool,
    pub launch_timer: f32,
    pub kills: u32,
    pub alarm_cd: f32,
    // eventos para el director de cámaras
    pub ev_missile: bool,
    pub ev_cluster: bool,
    pub ev_kill: bool,
    pub ev_bighit: bool,
    pub ev_zone: bool,
    pub ev_lock: bool,
    /// Vida de la nave nodriza terrestre (se conserva entre zonas).
    pub carrier_hp: f32,
    pub carrier_alert_cd: f32,
    /// Tiempo hasta que el anillo de recarga vuelva a estar disponible.
    pub rearm_cd: f32,
    pub rearm_flash: f32,
    /// true justo después de crear/reiniciar la partida (lo consume el sistema de misiones).
    pub fresh: bool,
    /// Oleadas automáticas (las misiones las encienden o apagan).
    pub auto_waves: bool,
    /// Interferencia de radar (segundos restantes).
    pub jam: f32,
    pub turret_kills: u32,
    /// Escudo de la nave nodriza Invid activo.
    pub mother_shield: bool,
    /// Distancia del plasma guiado más cercano (0 = ninguno).
    pub plasma_warn: f32,
    /// Multiplicador de daño recibido según la forma (CAZA / GERWALK / BATTROID).
    pub armor: f32,
    pub hive_alive: bool,
    pub ev_kill_pos: Option<(Vec3, Vec3)>,
}

impl Default for Game {
    fn default() -> Self {
        Self {
            started: false,
            paused: false,
            zone: Zone::Atmosphere,
            pending_zone: None,
            reset_on_switch: false,
            fade: 1.0,
            fade_dir: -1.0,
            score: 0,
            wave: 0,
            wave_timer: 3.0,
            hull: 100.0,
            shield: 100.0,
            shield_delay: 0.0,
            missiles: 8,
            missile_reload: 0.0,
            missile_cd: 0.0,
            cluster: 3,
            cluster_reload: 0.0,
            dead: false,
            msg: String::new(),
            msg_timer: 0.0,
            shake: 0.0,
            hit_flash: 0.0,
            gun_cd: 0.0,
            gun_side: false,
            heat: 0.0,
            overheated: false,
            lock: None,
            lock_time: 0.0,
            locked: false,
            show_help: true,
            mothership_alive: true,
            launch_timer: 10.0,
            kills: 0,
            alarm_cd: 0.0,
            ev_missile: false,
            ev_cluster: false,
            ev_kill: false,
            ev_bighit: false,
            ev_zone: false,
            ev_lock: false,
            carrier_hp: CARRIER_HP,
            carrier_alert_cd: 0.0,
            rearm_cd: 0.0,
            rearm_flash: 0.0,
            fresh: true,
            auto_waves: true,
            jam: 0.0,
            turret_kills: 0,
            mother_shield: false,
            plasma_warn: 0.0,
            armor: 1.0,
            hive_alive: true,
            ev_kill_pos: None,
        }
    }
}

impl Game {
    pub fn message(&mut self, s: impl Into<String>, t: f32) {
        self.msg = s.into();
        self.msg_timer = t;
    }
    pub fn damage(&mut self, amount: f32) {
        let amount = amount * self.armor;
        self.shield_delay = 3.5;
        self.hit_flash = (self.hit_flash + amount / 25.0).min(0.8);
        self.shake = (self.shake + amount / 18.0).min(1.5);
        if amount >= 15.0 {
            self.ev_bighit = true;
        }
        let absorbed = amount.min(self.shield);
        self.shield -= absorbed;
        self.hull -= amount - absorbed;
    }
}

#[derive(Component)]
pub struct Player {
    pub speed: f32,
    pub throttle: f32,
    pub ang_vel: Vec3, // pitch, yaw, roll (rad/s)
    pub boosting: bool,
    pub input: Vec3,
}

#[derive(Component)]
pub struct WorldCam;

#[derive(Component)]
pub struct CockpitRoot;

#[derive(Component)]
pub struct PlayerModel;

#[derive(Component)]
pub struct BlinkLight(pub f32);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EnemyKind {
    Fighter,
    Turret,
    Mothership,
    /// Colmena Invid terrestre.
    Hive,
}

#[derive(Component)]
pub struct Enemy {
    pub kind: EnemyKind,
    pub hp: f32,
    #[allow(dead_code)]
    pub max_hp: f32,
    pub radius: f32,
    pub half_extents: Option<Vec3>,
    pub cd: f32,
    pub speed: f32,
    pub vel: Vec3,
    pub evade: f32,
    pub evade_dir: Vec3,
    pub score: u32,
    pub hit_flash: f32,
}

#[derive(Component)]
pub struct MothershipTurret;

#[derive(Component)]
pub struct Bullet {
    pub vel: Vec3,
    pub prev: Vec3,
    pub life: f32,
    pub damage: f32,
    pub from_player: bool,
}

#[derive(Component)]
pub struct Missile {
    pub target: Option<Entity>,
    pub vel: Vec3,
    pub life: f32,
    pub smoke: f32,
    /// Micromisil del racimo (trayectoria curva con zigzag).
    pub cluster: bool,
    pub age: f32,
    pub seed: f32,
    pub damage: f32,
    /// Dirección hacia la que vuelan los micromisiles si no tienen blanco.
    pub aim: Vec3,
}

impl Missile {
    pub fn direct(target: Option<Entity>, vel: Vec3) -> Self {
        Self { target, vel, life: 7.0, smoke: 0.0, cluster: false, age: 0.0, seed: 0.0, damage: 170.0, aim: Vec3::ZERO }
    }
}

#[derive(Clone, Copy)]
pub enum EffectKind {
    Fireball { size: f32 },
    Smoke { size: f32 },
    Debris { vel: Vec3, spin: Vec3 },
    /// Restos de un Invid (sólo controla cuánto dura).
    Wreck,
}

#[derive(Component)]
pub struct Effect {
    pub kind: EffectKind,
    pub t: f32,
    pub life: f32,
}

#[derive(Component)]
pub struct Rock {
    pub radius: f32,
    pub spin: Vec3,
}

#[derive(Component)]
pub struct SpaceDust;

#[derive(Component)]
pub struct ZoneEntity;

#[derive(Component)]
pub struct Ground;

#[derive(Component)]
pub struct EarthClouds;

#[derive(Component)]
pub struct StarField;

#[derive(Component)]
pub struct SunDisc;

#[derive(Component)]
pub struct SunLight;

#[derive(Component)]
pub struct SkyDome;

#[derive(Component)]
pub struct Galaxy;

#[derive(Component)]
pub struct EngineGlow;

/// Partes del piloto que se ocultan en la vista de sus propios ojos.
#[derive(Component)]
pub struct PilotBody;

/// Vistas de cámara cinematográficas.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CamView {
    Pov,
    Shoulder,
    FacePilot,
    Chase,
    Wing,
    Flyby,
    StickClose,
    EnemyClose,
    KillCam,
}

impl CamView {
    pub const CYCLE: [CamView; 6] = [CamView::Pov, CamView::Shoulder, CamView::FacePilot, CamView::Chase, CamView::Wing, CamView::Flyby];
    pub fn name(self) -> &'static str {
        match self {
            CamView::Pov => "OJOS DEL PILOTO",
            CamView::Shoulder => "SOBRE EL HOMBRO",
            CamView::FacePilot => "CÁMARA AL PILOTO",
            CamView::Chase => "PERSECUCIÓN",
            CamView::Wing => "CÁMARA LATERAL",
            CamView::Flyby => "PASO DE CÁMARA",
            CamView::StickClose => "PRIMER PLANO",
            CamView::EnemyClose => "BLANCO FIJADO",
            CamView::KillCam => "DERRIBO",
        }
    }
    pub fn interior(self) -> bool {
        matches!(self, CamView::Pov | CamView::Shoulder | CamView::FacePilot | CamView::StickClose)
    }
}

/// Director: elige la vista y hace cortes cinematográficos ante eventos.
#[derive(Resource)]
pub struct Director {
    pub view: CamView,
    pub auto: bool,
    pub cut: Option<(CamView, f32)>,
    pub flyby_anchor: Vec3,
    pub flyby_set: bool,
    pub chase_pos: Vec3,
    pub chase_rot: Quat,
    pub slowmo: f32,
    pub cam: Transform,
    pub kill_pos: Vec3,
    pub kill_anchor: Vec3,
    pub kill_vel: Vec3,
    pub kill_t: f32,
    pub close_target: Option<Entity>,
}

impl Default for Director {
    fn default() -> Self {
        Self {
            view: CamView::Pov,
            auto: true,
            cut: None,
            flyby_anchor: Vec3::ZERO,
            flyby_set: false,
            chase_pos: Vec3::ZERO,
            chase_rot: Quat::IDENTITY,
            slowmo: 0.0,
            cam: Transform::default(),
            kill_pos: Vec3::ZERO,
            kill_anchor: Vec3::ZERO,
            kill_vel: Vec3::ZERO,
            kill_t: 0.0,
            close_target: None,
        }
    }
}

impl Director {
    pub fn current(&self) -> CamView {
        self.cut.map(|c| c.0).unwrap_or(self.view)
    }
}

/// Manos y antebrazos simplificados que sólo se ven desde los ojos del piloto.
#[derive(Component)]
pub struct PovOnly;

pub const CARRIER_HP: f32 = 4000.0;

/// Nave nodriza terrestre (portanaves): caja de colisión en su espacio local.
#[derive(Component)]
pub struct CarrierShip {
    pub half: Vec3,
    pub gun_cd: f32,
}

/// Anillo de recarga de armamento junto a la nave nodriza terrestre.
#[derive(Component)]
pub struct RearmRing;

/// Los cazas Invid con este componente atacan a la nave nodriza terrestre.
#[derive(Component)]
pub struct AttackCarrier {
    /// Punto del casco al que apuntan (espacio local del portanaves).
    pub aim: Vec3,
}
