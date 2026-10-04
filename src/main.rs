//! Road Intersection — перекрёсток со светофорами в изометрии.
//! Стрелки: выпустить машину с нужной стороны, R — случайную, Esc — выход.
//! Справа — админ-панель (admin.rs), слева сверху — статистика (stats.rs).
mod admin;
mod stats;
mod ui;
use sdl2::{event::Event, image::{InitFlag, LoadTexture}, keyboard::Keycode, pixels::Color, rect::Rect, render::{BlendMode, Canvas}, video::Window};
use admin::Admin;
use stats::{Snap, Stats};
use ui::SIDE_ARROW;
use std::{f32::consts::FRAC_PI_2 as Q, time::{Duration, Instant, SystemTime, UNIX_EPOCH}};

// Логический размер окна: сцена 1280 px + боковая панель справа
pub const SCENE_W: i32 = 1280;
pub const LW: i32 = 1580;
pub const LH: u32 = 720;

// ───────────── Симуляция (мир 800×800, логика «сверху») ─────────────
const WORLD: f32 = 800.0;
const C: f32 = WORLD / 2.0;
const LANE_W: f32 = 40.0;
const LANE_LEN: f32 = C - LANE_W;
const CAR_L: f32 = 30.0;
const CAR_W: f32 = 16.0;
const GAP: f32 = 10.0;
const SPEED: f32 = 120.0;
const CLEARANCE: f32 = CAR_L + GAP + 5.0;
const MIN_GREEN: f32 = 2.0;
const MAX_GREEN: f32 = 5.0;
const ALL_RED: f32 = 0.5;
pub const CAP: usize = (LANE_LEN / (CAR_L + GAP)) as usize;
const STOP_S: f32 = LANE_LEN - CAR_L / 2.0;

type V = (f32, f32);
fn add(a: V, b: V) -> V { (a.0 + b.0, a.1 + b.1) }
fn mul(a: V, k: f32) -> V { (a.0 * k, a.1 * k) }
fn rt(v: V) -> V { (-v.1, v.0) }
fn lf(v: V) -> V { (v.1, -v.0) }

/// Направления: 0 North, 1 South, 2 East, 3 West (куда едет машина).
const HEAD: [V; 4] = [(0., 1.), (0., -1.), (-1., 0.), (1., 0.)];
/// Манёвры: 0 налево, 1 прямо, 2 направо — радиусы дуг.
const RAD: [f32; 3] = [LANE_W * 1.5, 0., LANE_W / 2.];

fn lane(d: usize) -> V { add((C, C), mul(rt(HEAD[d]), LANE_W / 2.)) }
fn stop_pt(d: usize) -> V { add(lane(d), mul(HEAD[d], -LANE_W)) }

/// Позиция и направление машины на пути `s` по маршруту `r` с полосы `d`.
fn pose(d: usize, r: usize, s: f32) -> (V, V) {
    let (h, rad) = (HEAD[d], RAD[r]);
    if r == 1 || s <= LANE_LEN { return (add(lane(d), mul(h, s - C)), h); }
    let o = if r == 2 { rt(h) } else { lf(h) };
    let (arc, st) = (Q * rad, stop_pt(d));
    if s <= LANE_LEN + arc {
        let t = (s - LANE_LEN) / rad;
        let p = add(add(add(st, mul(o, rad)), mul(o, -rad * t.cos())), mul(h, rad * t.sin()));
        (p, add(mul(h, t.cos()), mul(o, t.sin())))
    } else {
        (add(add(st, add(mul(h, rad), mul(o, rad))), mul(o, s - LANE_LEN - arc)), o)
    }
}

struct Car { d: usize, r: usize, kind: usize, s: f32, wait: f32 }
impl Car {
    fn pos(&self) -> (V, V) { pose(self.d, self.r, self.s) }
    fn crossed(&self) -> bool { self.s > STOP_S }
    fn len(&self) -> f32 { if self.r == 1 { WORLD } else { 2. * LANE_LEN + Q * RAD[self.r] } }
    fn in_box(&self) -> bool {
        let (p, l) = (self.pos().0, LANE_W + CAR_L / 2.);
        (p.0 - C).abs() < l && (p.1 - C).abs() < l
    }
}

/// Режим светофора.
#[derive(Clone, Copy, PartialEq)]
pub enum Mode { Auto, Forced(usize), Stop, Flash }

impl Mode {
    fn short(self) -> &'static str {
        match self { Mode::Auto => "АВТО", Mode::Forced(_) => "РУЧНОЙ", Mode::Stop => "СТОП", Mode::Flash => "МИГАНИЕ" }
    }
}

pub struct World {
    cars: Vec<Car>, green: Option<usize>, last: Option<usize>, t: f32, rng: u64,
    pub mode: Mode,
    pub speed_mul: f32,
    pub route_pref: Option<usize>,
    pub kind_pref: Option<usize>,
    pub stats: Stats,
}
impl World {
    fn new(seed: u64) -> Self {
        World { cars: vec![], green: None, last: None, t: 0., rng: seed | 1, mode: Mode::Auto, speed_mul: 1.,
                route_pref: None, kind_pref: None, stats: Stats::new() }
    }

    /// Очистить дорогу и статистику (настройки остаются).
    pub fn reset(&mut self) {
        self.cars.clear();
        (self.green, self.last, self.t) = (None, None, 0.);
        let vis = self.stats.visible;
        self.stats = Stats::new();
        self.stats.visible = vis;
    }

    fn rand(&mut self, n: usize) -> usize {
        let mut x = self.rng;
        x ^= x << 13; x ^= x >> 7; x ^= x << 17;
        self.rng = x;
        ((x >> 11) % n as u64) as usize
    }

    pub fn spawn(&mut self, d: usize) -> bool {
        let rr = self.rand(3);
        let r = self.route_pref.unwrap_or(rr);
        let free = !self.cars.iter().any(|c| c.d == d && c.s < CLEARANCE);
        if free {
            let kind = self.kind_pref.unwrap_or(r);
            self.cars.push(Car { d, r, kind, s: 0., wait: 0. });
            self.stats.spawned[d] += 1;
        } else {
            self.stats.refused += 1;
        }
        free
    }

    pub fn spawn_random(&mut self) { let d = self.rand(4); self.spawn(d); }

    /// Длина очереди (машины до стоп-линии) по каждой стороне.
    fn queues(&self) -> [usize; 4] {
        let mut q = [0usize; 4];
        for c in self.cars.iter().filter(|c| !c.crossed()) { q[c.d] += 1; }
        q
    }

    fn release(&mut self) { (self.last, self.green, self.t) = (self.green, None, 0.); }

    /// Светофор: зелёный одной стороне, между фазами «все красные».
    fn lights(&mut self, dt: f32) {
        let q = self.queues();
        let empty = !self.cars.iter().any(|c| c.in_box());
        self.t += dt;
        match self.mode {
            Mode::Auto => self.auto_phase(q, empty),
            // Ручной зелёный: сначала гасим текущий, ждём пока перекрёсток опустеет.
            Mode::Forced(d) => match self.green {
                Some(g) if g == d => {}
                Some(_) => self.release(),
                None => if self.t >= ALL_RED && empty { (self.green, self.t) = (Some(d), 0.); }
            },
            Mode::Stop => if self.green.is_some() { self.release(); },
            // Мигающий режим: по одной машине, по кругу (как нерегулируемый перекрёсток).
            Mode::Flash => match self.green {
                Some(cur) => {
                    let entered = self.cars.iter().any(|c| c.d == cur && c.crossed() && c.in_box());
                    if q[cur] == 0 || entered || self.t >= 3. { self.release(); }
                }
                None if self.t >= ALL_RED && empty => {
                    let start = self.last.map_or(0, |l| l + 1);
                    if let Some(d) = (0..4).map(|k| (start + k) % 4).find(|&d| q[d] > 0) { (self.green, self.t) = (Some(d), 0.); }
                }
                None => {}
            },
        }
    }

    fn auto_phase(&mut self, q: [usize; 4], empty: bool) {
        match self.green {
            Some(cur) => {
                let full = q[cur] >= CAP;
                let other_full = (0..4).any(|d| d != cur && q[d] >= CAP);
                let limit = if full { MAX_GREEN * 2. } else { MAX_GREEN };
                if self.t >= MIN_GREEN && (q[cur] == 0 || (other_full && !full) || self.t >= limit) { self.release(); }
            }
            None if self.t >= ALL_RED && empty => {
                let pick = |ex: Option<usize>| (0..4).filter(|&d| Some(d) != ex && q[d] > 0).max_by_key(|&d| (q[d] >= CAP, q[d]));
                if let Some(d) = pick(self.last).or_else(|| pick(None)) { (self.green, self.t) = (Some(d), 0.); }
            }
            None => {}
        }
    }

    fn update(&mut self, dt: f32) {
        self.lights(dt);
        self.stats.tick(dt);
        self.cars.sort_by(|a, b| b.s.partial_cmp(&a.s).unwrap());
        let (green, step) = (self.green, SPEED * self.speed_mul * dt);
        let mut lead = [None::<f32>; 4];
        for c in &mut self.cars {
            let before = c.s;
            let mut n = c.s + step;
            // не наезжать на переднюю машину (с запасом, чтобы быстрый режим не «проскакивал»)
            if let Some(l) = lead[c.d] { n = n.min((l - CAR_L - GAP).max(c.s)); }
            if green != Some(c.d) && !c.crossed() { n = n.min(STOP_S); }
            c.s = n;
            if !c.crossed() && c.s - before < 0.01 { c.wait += dt; }
            lead[c.d] = Some(c.s);
        }
        let stats = &mut self.stats;
        self.cars.retain(|c| {
            let keep = c.s < c.len() + CAR_L;
            if !keep { stats.on_exit(c.d, c.wait); }
            keep
        });
    }
}

// ───────────── Графика (изометрия поверх той же логики) ─────────────
const K: f32 = 0.8;
pub type Col = (u8, u8, u8, u8);
const CAR_COLORS: [Col; 4] = [(205, 85, 70, 255), (70, 140, 190, 255), (235, 190, 75, 255), (228, 228, 222, 255)];

fn proj(x: f32, y: f32, z: f32) -> V { ((x - y) * K + 640., (x + y - WORLD) * K / 2. + 372. - z * K * 1.2) }
fn sh(c: Col, f: f32) -> Col { let m = |v: u8| (v as f32 * f).min(255.) as u8; (m(c.0), m(c.1), m(c.2), c.3) }

struct G { c: Canvas<Window>, a: f32 }
impl G {
    /// Закрашенный выпуклый многоугольник (построчно).
    fn poly(&mut self, c: Col, p: &[V]) {
        self.c.set_draw_color(Color::RGBA(c.0, c.1, c.2, (c.3 as f32 * self.a) as u8));
        let (y0, y1) = p.iter().fold((f32::MAX, f32::MIN), |(a, b), q| (a.min(q.1), b.max(q.1)));
        for y in y0.ceil() as i32..=y1.floor() as i32 {
            let (mut lo, mut hi) = (f32::MAX, f32::MIN);
            for i in 0..p.len() {
                let (a, b) = (p[i], p[(i + 1) % p.len()]);
                if (a.1 <= y as f32) != (b.1 <= y as f32) {
                    let x = a.0 + (y as f32 - a.1) * (b.0 - a.0) / (b.1 - a.1);
                    lo = lo.min(x); hi = hi.max(x);
                }
            }
            if lo <= hi { let _ = self.c.draw_line((lo.round() as i32, y), (hi.round() as i32, y)); }
        }
    }

    fn corners(p: V, f: V, len: f32, wid: f32) -> Vec<V> {
        [(1., 1.), (1., -1.), (-1., -1.), (-1., 1.)].iter().map(|&(a, b)| add(p, add(mul(f, a * len / 2.), mul(rt(f), b * wid / 2.)))).collect()
    }

    /// Плоский прямоугольник на земле: центр, направление, длина, ширина.
    fn quad(&mut self, c: Col, p: V, f: V, len: f32, wid: f32) {
        let pts: Vec<V> = Self::corners(p, f, len, wid).iter().map(|q| proj(q.0, q.1, 0.)).collect();
        self.poly(c, &pts);
    }

    fn rect(&mut self, c: Col, x0: f32, y0: f32, x1: f32, y1: f32) {
        self.quad(c, ((x0 + x1) / 2., (y0 + y1) / 2.), (1., 0.), x1 - x0, y1 - y0);
    }

    /// Параллелепипед: видимые боковые грани + крышка.
    fn cuboid(&mut self, c: Col, top: Col, p: V, f: V, len: f32, wid: f32, z0: f32, z1: f32) {
        let cs = Self::corners(p, f, len, wid);
        for i in 0..4 {
            let (a, b) = (cs[i], cs[(i + 1) % 4]);
            let n = ((a.0 + b.0) / 2. - p.0, (a.1 + b.1) / 2. - p.1);
            if n.0 + n.1 > 0.01 {
                let s = 0.7 + 0.25 * (n.0 - n.1) / (n.0.abs() + n.1.abs());
                self.poly(sh(c, s), &[proj(a.0, a.1, z0), proj(b.0, b.1, z0), proj(b.0, b.1, z1), proj(a.0, a.1, z1)]);
            }
        }
        self.poly(top, &cs.iter().map(|q| proj(q.0, q.1, z1)).collect::<Vec<_>>());
    }

    /// Земля: фон, плита-«диорама», тротуары, кварталы, дороги и разметка.
    fn ground(&mut self, tints: [Col; 4]) {
        for i in 0..18 {
            self.c.set_draw_color(Color::RGB(20 + i, 36 + i, 46 + i));
            let _ = self.c.fill_rect(Rect::new(0, i as i32 * 40, LW as u32, 40));
        }
        let (t, r, b, l) = (proj(0., 0., 0.), proj(WORLD, 0., 0.), proj(WORLD, WORLD, 0.), proj(0., WORLD, 0.));
        let down = |p: V| (p.0, p.1 + 24.);
        self.poly((92, 66, 48, 255), &[l, b, down(b), down(l)]);
        self.poly((124, 92, 66, 255), &[b, r, down(r), down(b)]);
        self.poly((104, 134, 86, 255), &[t, r, b, l]);
        let (sand, plaza, curb) = ((204, 178, 136, 255), (190, 162, 120, 255), (146, 120, 88, 255));
        self.rect(sand, C - 54., 0., C + 54., WORLD);
        self.rect(sand, 0., C - 54., WORLD, C + 54.);
        for (x, y) in [(10., 10.), (454., 10.), (10., 454.), (454., 454.)] {
            self.rect(curb, x - 3., y - 3., x + 339., y + 339.);
            self.rect(plaza, x, y, x + 336., y + 336.);
        }
        let (asphalt, yellow, white) = ((58, 60, 67, 255), (224, 180, 50, 255), (234, 232, 224, 255));
        self.rect(asphalt, C - 40., 0., C + 40., WORLD);
        self.rect(asphalt, 0., C - 40., WORLD, C + 40.);
        for d in 0..4 {
            let (h, arm) = (HEAD[d], add((C, C), mul(HEAD[d], -220.)));
            for s in [-1.6, 1.6] { self.quad(yellow, add(arm, mul(rt(h), s)), h, 360., 1.2); }
            for s in [-37., 37.] { self.quad(white, add(arm, mul(rt(h), s)), h, 360., 1.4); }
            for i in -4..=4 { self.quad(white, add((C, C), add(mul(h, -34.), mul(rt(h), i as f32 * 8.))), h, 12., 4.); }
            self.quad(white, add(lane(d), mul(h, -LANE_W - 2.)), h, 3., 36.);
            self.quad(tints[d], add(lane(d), mul(h, -LANE_W - 10.)), h, 14., 36.);
        }
    }

    fn light(&mut self, d: usize, base: Col, pulse: bool, time: f32) {
        let (h, st) = (HEAD[d], stop_pt(d));
        let lamp = add(add(st, mul(rt(h), 30.)), mul(h, -10.));
        let glow = if pulse { 1. + 0.2 * (time * 6.).sin() } else { 1. };
        self.cuboid((46, 48, 54, 255), (70, 72, 80, 255), lamp, (1., 0.), 3., 3., 0., 26.);
        self.cuboid(sh(base, glow), sh(base, 1.2 * glow), lamp, (1., 0.), 8., 8., 26., 35.);
    }

    fn car(&mut self, c: &Car) {
        let (p, h) = c.pos();
        self.a = (c.s.min(c.len() + CAR_L - c.s) / 40.).clamp(0., 1.);
        let body = CAR_COLORS[c.kind];
        self.quad((0, 0, 0, 70), add(p, (3., 3.)), h, CAR_L, CAR_W);
        self.cuboid(body, sh(body, 1.12), p, h, CAR_L, CAR_W, 2., 8.);
        self.cuboid((40, 56, 74, 255), sh(body, 1.18), add(p, mul(h, -2.)), h, 14., 12., 8., 12.5);
        self.a = 1.;
    }
}

enum It { Car(usize), Light(usize), Block(usize) }

/// Цвет лампы и подсветки перед стоп-линией для стороны `d`.
fn lamp(w: &World, d: usize, time: f32) -> (Col, Col) {
    if w.mode == Mode::Flash {
        return if (time * 2.).fract() < 0.5 { ((255, 190, 40, 255), (255, 190, 40, 70)) } else { ((90, 70, 20, 255), (255, 190, 40, 0)) };
    }
    if w.green == Some(d) { ((60, 220, 90, 255), (60, 220, 90, 70)) } else { ((235, 55, 55, 255), (235, 55, 55, 40)) }
}

fn main() -> Result<(), String> {
    let sdl = sdl2::init()?;
    let _img = sdl2::image::init(InitFlag::PNG)?;
    sdl2::hint::set("SDL_RENDER_SCALE_QUALITY", "linear");
    let window = sdl.video()?
        .window("Road Intersection  |  стрелки: машина с края, R: случайная, Пробел: пауза, H: статистика, Tab: панель", 1400, 638)
        .position_centered().resizable().build().map_err(|e| e.to_string())?;
    let mut canvas = window.into_canvas().present_vsync().build().map_err(|e| e.to_string())?;
    canvas.set_logical_size(LW as u32, LH).map_err(|e| e.to_string())?;
    canvas.set_blend_mode(BlendMode::Blend);
    let tc = canvas.texture_creator();
    // Здания: файл, ширина на экране, ближний угол основания в мире
    let sp = [("university", 520., 346., 346.), ("market", 520., 790., 346.), ("mosque", 330., 225., 670.),
              ("house1", 130., 197., 785.), ("house2", 130., 341., 641.), ("shopping", 520., 790., 790.)];
    let tex = sp.iter().map(|b| tc.load_texture(format!("assets/{}.png", b.0))).collect::<Result<Vec<_>, _>>()?;
    let mut g = G { c: canvas, a: 1. };
    let mut events = sdl.event_pump()?;
    let mut world = World::new(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() as u64);
    let (mut last, mut time, mut fps) = (Instant::now(), 0.0f32, 60.0f32);
    let (mut admin, mut ui) = (Admin::new(), ui::Ui::new());

    'run: loop {
        for e in events.poll_iter() {
            ui.event(&e);
            match e {
                Event::Quit { .. } | Event::KeyDown { keycode: Some(Keycode::Escape), .. } => break 'run,
                Event::KeyDown { keycode: Some(k), repeat: false, .. } => match k {
                    Keycode::Up => { world.spawn(1); }
                    Keycode::Down => { world.spawn(0); }
                    Keycode::Left => { world.spawn(2); }
                    Keycode::Right => { world.spawn(3); }
                    Keycode::R => world.spawn_random(),
                    Keycode::Space | Keycode::P => admin.paused = !admin.paused,
                    Keycode::H => world.stats.visible = !world.stats.visible,
                    Keycode::Tab => admin.visible = !admin.visible,
                    _ => {}
                },
                _ => {}
            }
        }
        let raw = last.elapsed().as_secs_f32();
        let dt = raw.min(0.05);
        last = Instant::now();
        fps += (1. / raw.max(1e-4) - fps) * 0.1;
        time += dt;
        admin.auto_flow(dt, &mut world);
        if !admin.paused { world.update(dt); }

        g.ground([0, 1, 2, 3].map(|d| lamp(&world, d, time).1));
        // Рисуем всё от дальнего к ближнему
        let mut items: Vec<(f32, It)> = vec![];
        for (i, b) in sp.iter().enumerate() { items.push((b.2 + b.3 - b.1 / (2. * K), It::Block(i))); }
        for d in 0..4 {
            let l = add(add(stop_pt(d), mul(rt(HEAD[d]), 30.)), mul(HEAD[d], -10.));
            items.push((l.0 + l.1, It::Light(d)));
        }
        for (i, c) in world.cars.iter().enumerate() { let p = c.pos().0; items.push((p.0 + p.1, It::Car(i))); }
        items.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        for (_, it) in &items {
            match *it {
                It::Car(i) => g.car(&world.cars[i]),
                It::Light(d) => { let c = lamp(&world, d, time).0; g.light(d, c, world.green == Some(d) && world.mode != Mode::Flash, time) }
                It::Block(i) => {
                    let q = tex[i].query();
                    let (w, p) = (sp[i].1, proj(sp[i].2, sp[i].3, 0.));
                    let h = w * q.height as f32 / q.width as f32;
                    let _ = g.c.copy(&tex[i], None, Rect::new((p.0 - w / 2.) as i32, (p.1 - h) as i32, w as u32, h as u32));
                }
            }
        }
        // Интерфейс поверх сцены
        let queues = world.queues();
        world.stats.observe(&queues);
        let light = match (world.mode, world.green) {
            (Mode::Flash, _) => "МИГАНИЕ".to_string(),
            (Mode::Stop, _) => "СТОП ВСЕ КРАСН.".to_string(),
            (m, Some(d)) => format!("{} {} {:.1}С", m.short(), SIDE_ARROW[d], world.t),
            (m, None) => format!("{} ВСЕ КРАСН.", m.short()),
        };
        let snap = Snap { queues, on_road: world.cars.len(), in_box: world.cars.iter().filter(|c| c.in_box()).count(),
                          green: world.green, light, paused: admin.paused, fps };
        ui.begin();
        if admin.visible { admin.draw(&mut ui, &mut g.c, &mut world); }
        world.stats.draw(&mut g.c, &snap);
        ui.end();
        g.c.present();
        std::thread::sleep(Duration::from_millis(16));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(w: &mut World, secs: f32) { for _ in 0..(secs / 0.016) as usize { w.update(0.016); } }

    fn fill(w: &mut World) { for d in 0..4 { w.spawn(d); } }

    /// Минимальный зазор между машинами одной полосы (отрицательный = наезд).
    fn min_gap(w: &World) -> f32 {
        let mut m = f32::MAX;
        for a in &w.cars { for b in &w.cars { if a.d == b.d && b.s > a.s { m = m.min(b.s - a.s - CAR_L); } } }
        m
    }

    #[test]
    fn forced_green_only_for_chosen_side() {
        let mut w = World::new(7);
        w.mode = Mode::Forced(2);
        for _ in 0..40 { fill(&mut w); run(&mut w, 0.25); assert!(w.green.is_none() || w.green == Some(2)); }
        assert_eq!(w.green, Some(2));
        assert!(w.stats.passed[2] > 0 && w.stats.passed[0] == 0 && w.stats.passed[1] == 0 && w.stats.passed[3] == 0);
    }

    #[test]
    fn stop_never_gives_green() {
        let mut w = World::new(7);
        w.mode = Mode::Stop;
        for _ in 0..30 { fill(&mut w); run(&mut w, 0.25); assert!(w.green.is_none()); }
        assert_eq!(w.stats.total_passed(), 0);
    }

    #[test]
    fn flash_serves_all_sides_one_at_a_time() {
        let mut w = World::new(7);
        w.mode = Mode::Flash;
        for _ in 0..60 { fill(&mut w); run(&mut w, 0.5); }
        assert!((0..4).all(|d| w.stats.passed[d] > 0), "{:?}", w.stats.passed);
    }

    #[test]
    fn no_overlap_at_max_speed_and_reset_works() {
        let mut w = World::new(3);
        w.speed_mul = 3.;
        for _ in 0..200 { fill(&mut w); for _ in 0..3 { w.update(0.05); assert!(min_gap(&w) >= GAP - 0.01, "gap {}", min_gap(&w)); } }
        assert!(w.stats.total_passed() > 0 && w.stats.avg_wait() >= 0.);
        w.reset();
        assert!(w.cars.is_empty() && w.stats.total_passed() == 0 && w.green.is_none());
    }
}
