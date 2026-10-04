//! Админ-панель: боковая панель справа с управлением светофором, потоком и спавном.
use crate::{ui::*, Mode, World, CAR_COLORS, LW, LH, SCENE_W};
use sdl2::{rect::Rect, render::Canvas, video::Window};

pub const PW: i32 = LW - SCENE_W;

pub struct Admin {
    pub visible: bool,
    pub paused: bool,
    pub auto: bool,
    pub rate: f32,
    acc: f32,
}

impl Admin {
    pub fn new() -> Self { Admin { visible: true, paused: false, auto: false, rate: 1.0, acc: 0. } }

    /// Автопоток: выпускает случайные машины с заданной частотой.
    pub fn auto_flow(&mut self, dt: f32, w: &mut World) {
        if !self.auto || self.paused { self.acc = 0.; return; }
        self.acc += dt * self.rate;
        while self.acc >= 1. { self.acc -= 1.; w.spawn_random(); }
    }

    pub fn draw(&mut self, ui: &mut Ui, c: &mut Canvas<Window>, w: &mut World) {
        let (px, x, wf) = (SCENE_W, SCENE_W + 12, PW - 24);
        fill(c, Rect::new(px, 0, PW as u32, LH), (22, 25, 34, 255));
        fill(c, Rect::new(px, 0, 2, LH), (84, 92, 116, 255));
        let mut y = 12;
        text(c, x, y, 2, GOLD, "АДМИН-ПАНЕЛЬ");
        y += 34;

        // ── Светофор
        text(c, x, y, 2, DIM, "СВЕТОФОР"); y += 20;
        let bw = (wf - 12) / 3;
        for (i, (l, m)) in [("АВТО", Mode::Auto), ("СТОП", Mode::Stop), ("МИГАНИЕ", Mode::Flash)].iter().enumerate() {
            if ui.button(c, Rect::new(x + i as i32 * (bw + 6), y, bw as u32, 28), l, w.mode == *m) { w.mode = *m; }
        }
        y += 38;
        text(c, x, y, 2, DIM, "ЗЕЛЁНЫЙ ВРУЧНУЮ"); y += 20;
        let bw = (wf - 18) / 4;
        for (i, &d) in SIDE_ORDER.iter().enumerate() {
            let r = Rect::new(x + i as i32 * (bw + 6), y, bw as u32, 28);
            if ui.button(c, r, &SIDE_ARROW[d].to_string(), w.mode == Mode::Forced(d)) { w.mode = Mode::Forced(d); }
        }
        y += 44;

        // ── Движение
        text(c, x, y, 2, DIM, "ДВИЖЕНИЕ"); y += 20;
        ui.slider(c, Rect::new(x, y, wf as u32, 28), &format!("СКОРОСТЬ X{:.1}", w.speed_mul), &mut w.speed_mul, 0.25, 3.0);
        y += 36;
        let bw = (wf - 6) / 2;
        if ui.button(c, Rect::new(x, y, bw as u32, 28), if self.paused { "ПУСК" } else { "ПАУЗА" }, self.paused) { self.paused = !self.paused; }
        if ui.button(c, Rect::new(x + bw + 6, y, bw as u32, 28), "СБРОС", false) { w.reset(); }
        y += 44;

        // ── Автопоток
        text(c, x, y, 2, DIM, "АВТОПОТОК"); y += 20;
        if ui.button(c, Rect::new(x, y, wf as u32, 28), if self.auto { "ВКЛЮЧЕН" } else { "ВЫКЛЮЧЕН" }, self.auto) { self.auto = !self.auto; }
        y += 36;
        ui.slider(c, Rect::new(x, y, wf as u32, 28), &format!("ЧАСТОТА {:.1}/С", self.rate), &mut self.rate, 0.2, 4.0);
        y += 44;

        // ── Что выпускать
        text(c, x, y, 2, DIM, "МАРШРУТ"); y += 20;
        let routes = [("СЛУЧАЙНО", None), ("НАЛЕВО", Some(0)), ("ПРЯМО", Some(1)), ("НАПРАВО", Some(2))];
        let bw = (wf - 6) / 2;
        for (i, (l, v)) in routes.iter().enumerate() {
            let r = Rect::new(x + (i as i32 % 2) * (bw + 6), y + (i as i32 / 2) * 34, bw as u32, 28);
            if ui.button(c, r, l, w.route_pref == *v) { w.route_pref = *v; }
        }
        y += 74;
        text(c, x, y, 2, DIM, "ЦВЕТ МАШИНЫ"); y += 20;
        let bw = (wf - 24) / 5;
        if ui.button(c, Rect::new(x, y, bw as u32, 28), "АВТО", w.kind_pref.is_none()) { w.kind_pref = None; }
        for (i, col) in CAR_COLORS.iter().enumerate() {
            let r = Rect::new(x + (i as i32 + 1) * (bw + 6), y, bw as u32, 28);
            if ui.swatch(c, r, *col, w.kind_pref == Some(i)) { w.kind_pref = Some(i); }
        }
        y += 44;
        text(c, x, y, 2, DIM, "ВЫПУСТИТЬ С"); y += 20;
        let bw = (wf - 18) / 4;
        for (i, &d) in SIDE_ORDER.iter().enumerate() {
            let r = Rect::new(x + i as i32 * (bw + 6), y, bw as u32, 28);
            if ui.button(c, r, &SIDE_ARROW[d].to_string(), false) { w.spawn(d); }
        }
        y += 34;
        if ui.button(c, Rect::new(x, y, wf as u32, 28), "СЛУЧАЙНАЯ (R)", false) { w.spawn_random(); }

        // ── Подсказки по клавишам
        let mut hy = LH as i32 - 76;
        for line in ["ПРОБЕЛ: ПАУЗА", "H: СТАТИСТИКА", "TAB: СКРЫТЬ ПАНЕЛЬ", "СТРЕЛКИ: ВЫПУСК МАШИН"] {
            text(c, x, hy, 2, (100, 110, 135, 255), line);
            hy += 17;
        }
    }
}
