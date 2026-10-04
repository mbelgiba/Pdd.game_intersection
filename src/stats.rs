//! Модуль статистики: сбор показателей и HUD в левом верхнем углу.
use crate::{ui::*, Col, CAP};
use sdl2::{rect::Rect, render::Canvas, video::Window};
use std::collections::VecDeque;

/// Срез состояния мира для отрисовки (собирается в main каждый кадр).
pub struct Snap {
    pub queues: [usize; 4],
    pub on_road: usize,
    pub in_box: usize,
    pub green: Option<usize>,
    pub light: String,
    pub paused: bool,
    pub fps: f32,
}

pub struct Stats {
    pub visible: bool,
    pub elapsed: f32,
    pub spawned: [u32; 4],
    pub passed: [u32; 4],
    pub refused: u32,
    pub max_queue: [usize; 4],
    wait_sum: f32,
    wait_max: f32,
    recent: VecDeque<f32>,
}

impl Stats {
    pub fn new() -> Self {
        Stats { visible: true, elapsed: 0., spawned: [0; 4], passed: [0; 4], refused: 0, max_queue: [0; 4],
                wait_sum: 0., wait_max: 0., recent: VecDeque::new() }
    }

    /// Течение времени симуляции.
    pub fn tick(&mut self, dt: f32) {
        self.elapsed += dt;
        while self.recent.front().map_or(false, |&t| t < self.elapsed - 60.) { self.recent.pop_front(); }
    }

    /// Машина покинула мир; `wait` — сколько она простояла до проезда стоп-линии.
    pub fn on_exit(&mut self, d: usize, wait: f32) {
        self.passed[d] += 1;
        self.wait_sum += wait;
        self.wait_max = self.wait_max.max(wait);
        self.recent.push_back(self.elapsed);
    }

    pub fn observe(&mut self, queues: &[usize; 4]) {
        for d in 0..4 { self.max_queue[d] = self.max_queue[d].max(queues[d]); }
    }

    pub fn total_passed(&self) -> u32 { self.passed.iter().sum() }
    pub fn total_spawned(&self) -> u32 { self.spawned.iter().sum() }
    pub fn avg_wait(&self) -> f32 { let n = self.total_passed(); if n == 0 { 0. } else { self.wait_sum / n as f32 } }
    /// Пропускная способность, машин в минуту (по последней минуте).
    pub fn per_min(&self) -> f32 { self.recent.len() as f32 / self.elapsed.clamp(5., 60.) * 60. }

    pub fn draw(&self, c: &mut Canvas<Window>, s: &Snap) {
        if !self.visible { return; }
        let (x, w, lh) = (10, 340, 17);
        let h = 14 + 10 * lh + 8 + lh + 4 * 22 + 8;
        fill(c, Rect::new(x, 10, w as u32, h as u32), (12, 16, 24, 210));
        outline(c, Rect::new(x, 10, w as u32, h as u32), (84, 92, 116, 255));
        let (l, r) = (x + 10, x + w - 10);
        let mut y = 18;

        text(c, l, y, 2, GOLD, "СТАТИСТИКА");
        text_r(c, r, y, 2, DIM, &format!("{:.0} FPS", s.fps));
        y += lh + 2;

        let row = |c: &mut Canvas<Window>, y: i32, k: &str, v: &str, col: Col| {
            text(c, l, y, 2, DIM, k);
            text_r(c, r, y, 2, col, v);
        };
        let secs = self.elapsed as u32;
        let time = format!("{:02}:{:02}{}", secs / 60, secs % 60, if s.paused { " ПАУЗА" } else { "" });
        row(c, y, "ВРЕМЯ", &time, if s.paused { GOLD } else { WHITE }); y += lh;
        row(c, y, "СВЕТОФОР", &s.light, WHITE); y += lh;
        row(c, y, "НА ДОРОГЕ", &format!("{} (ЦЕНТР {})", s.on_road, s.in_box), WHITE); y += lh;
        row(c, y, "ВЫПУЩЕНО", &self.total_spawned().to_string(), WHITE); y += lh;
        row(c, y, "ПРОЕХАЛО", &self.total_passed().to_string(), (120, 230, 120, 255)); y += lh;
        row(c, y, "ПОТОК", &format!("{:.0} МАШ/МИН", self.per_min()), WHITE); y += lh;
        row(c, y, "СР. ОЖИДАНИЕ", &format!("{:.1} С", self.avg_wait()), WHITE); y += lh;
        row(c, y, "МАКС. ОЖИДАНИЕ", &format!("{:.1} С", self.wait_max), WHITE); y += lh;
        row(c, y, "ОТКАЗОВ", &self.refused.to_string(), if self.refused > 0 { (255, 120, 100, 255) } else { WHITE }); y += lh + 8;

        text(c, l + 36, y, 2, DIM, "ОЧЕРЕДЬ");
        text_r(c, r, y, 2, DIM, "ПРОЕХАЛО");
        y += lh;
        for &d in SIDE_ORDER.iter() {
            let q = s.queues[d];
            let lamp = if s.green == Some(d) { (60, 220, 90, 255) } else { (235, 55, 55, 255) };
            fill(c, Rect::new(l, y + 2, 10, 10), lamp);
            text(c, l + 18, y, 2, SIDE_COL[d], &SIDE_ARROW[d].to_string());
            let bar = Rect::new(l + 46, y + 1, 110, 12);
            fill(c, bar, (45, 50, 64, 255));
            let full = q >= CAP;
            fill(c, Rect::new(bar.x, bar.y, (110 * q.min(CAP) / CAP) as u32, 12), if full { (235, 80, 70, 255) } else { SIDE_COL[d] });
            outline(c, bar, (84, 92, 116, 255));
            text(c, l + 164, y, 2, WHITE, &format!("{}/{}", q, CAP));
            text_r(c, r, y, 2, WHITE, &self.passed[d].to_string());
            // пик очереди — под основной строкой мелким шрифтом
            text(c, l + 46, y + 14, 1, DIM, &format!("ПИК {}", self.max_queue[d]));
            y += 22;
        }
    }
}
