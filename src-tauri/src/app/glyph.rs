//! Procedurally drawn 32x32 tray glyphs (Wi-Fi / USB) so no image decoder or asset is needed.
use tauri::image::Image;

const SIZE: usize = 32;
const BG: [f32; 3] = [24.0, 28.0, 40.0];

pub const GREEN: [f32; 3] = [70.0, 220.0, 140.0];
pub const AMBER: [f32; 3] = [250.0, 190.0, 60.0];

struct Canvas {
    px: Vec<[f32; 4]>,
}

impl Canvas {
    fn new() -> Self {
        let mut c = Self { px: vec![[0.0; 4]; SIZE * SIZE] };
        c.paint(BG, |x, y| coverage(15.5 - dist(x, y, 15.5, 15.5), 0.0));
        c
    }

    /// Alpha-composites `color` over the canvas with per-pixel coverage `f(x, y)`.
    fn paint(&mut self, color: [f32; 3], f: impl Fn(f32, f32) -> f32) {
        for y in 0..SIZE {
            for x in 0..SIZE {
                let a = f(x as f32 + 0.5, y as f32 + 0.5).clamp(0.0, 1.0);
                if a <= 0.0 {
                    continue;
                }
                let p = &mut self.px[y * SIZE + x];
                let out_a = a + p[3] * (1.0 - a);
                for i in 0..3 {
                    p[i] = (color[i] * a + p[i] * p[3] * (1.0 - a)) / out_a;
                }
                p[3] = out_a;
            }
        }
    }

    fn disc(&mut self, color: [f32; 3], cx: f32, cy: f32, r: f32) {
        self.paint(color, |x, y| coverage(r - dist(x, y, cx, cy), 0.0));
    }

    fn segment(&mut self, color: [f32; 3], a: (f32, f32), b: (f32, f32), half: f32) {
        self.paint(color, |x, y| coverage(half - seg_dist(x, y, a, b), 0.0));
    }

    fn triangle(&mut self, color: [f32; 3], p: [(f32, f32); 3]) {
        self.paint(color, |x, y| {
            let s = |a: (f32, f32), b: (f32, f32)| (b.0 - a.0) * (y - a.1) - (b.1 - a.1) * (x - a.0);
            let (d1, d2, d3) = (s(p[0], p[1]), s(p[1], p[2]), s(p[2], p[0]));
            let neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
            let pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
            if neg && pos { 0.0 } else { 1.0 }
        });
    }

    fn image(self) -> Image<'static> {
        let mut rgba = Vec::with_capacity(SIZE * SIZE * 4);
        for p in self.px {
            rgba.extend_from_slice(&[
                p[0].round().clamp(0.0, 255.0) as u8,
                p[1].round().clamp(0.0, 255.0) as u8,
                p[2].round().clamp(0.0, 255.0) as u8,
                (p[3] * 255.0).round().clamp(0.0, 255.0) as u8,
            ]);
        }
        Image::new_owned(rgba, SIZE as u32, SIZE as u32)
    }
}

fn dist(x: f32, y: f32, cx: f32, cy: f32) -> f32 {
    ((x - cx).powi(2) + (y - cy).powi(2)).sqrt()
}

fn coverage(signed: f32, edge: f32) -> f32 {
    signed - edge + 0.5
}

fn seg_dist(x: f32, y: f32, a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 == 0.0 { 0.0 } else { (((x - a.0) * dx + (y - a.1) * dy) / len2).clamp(0.0, 1.0) };
    dist(x, y, a.0 + t * dx, a.1 + t * dy)
}

pub fn wifi(color: [f32; 3]) -> Image<'static> {
    let mut c = Canvas::new();
    let (cx, cy) = (16.0, 24.0);
    c.disc(color, cx, cy, 2.4);
    for r in [7.0f32, 12.0, 17.0] {
        c.paint(color, |x, y| {
            let (dx, dy) = (x - cx, cy - y);
            let angle = dx.atan2(dy).abs();
            if dy <= 0.0 || angle > 0.85 {
                return 0.0;
            }
            coverage(1.2 - (dist(x, y, cx, cy) - r).abs(), 0.0)
        });
    }
    c.image()
}

pub fn usb(color: [f32; 3]) -> Image<'static> {
    let mut c = Canvas::new();
    c.segment(color, (16.0, 8.0), (16.0, 25.0), 1.2);
    c.triangle(color, [(16.0, 4.0), (12.5, 9.5), (19.5, 9.5)]);
    c.disc(color, 16.0, 26.0, 2.8);
    c.segment(color, (16.0, 19.0), (10.0, 15.0), 1.0);
    c.segment(color, (10.0, 15.0), (10.0, 12.5), 1.0);
    c.paint(color, |x, y| if (8.2..11.8).contains(&x) && (9.2..12.8).contains(&y) { 1.0 } else { 0.0 });
    c.segment(color, (16.0, 16.0), (22.0, 12.0), 1.0);
    c.segment(color, (22.0, 12.0), (22.0, 10.0), 1.0);
    c.disc(color, 22.0, 9.0, 2.2);
    c.image()
}
