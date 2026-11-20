// Layered-ridge wallpaper generator (std only). Writes binary PPM to stdout.
//
//   rustc -O -o wavegen main.rs
//   ./wavegen fern | cjpeg -quality 92 -optimize > fern.jpg
//
// Presets: meadow, fern, valley. Add one by copying a preset block.
use std::io::Write;

const W: usize = 2880;
const H: usize = 1800;

type C = [f32; 3];

fn hex(s: &str) -> C {
    let v = u32::from_str_radix(s, 16).unwrap();
    [((v >> 16) & 255) as f32 / 255.0, ((v >> 8) & 255) as f32 / 255.0, (v & 255) as f32 / 255.0]
}
fn mix(a: C, b: C, t: f32) -> C {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}
fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

struct Layer {
    base_y: f32,                       // ridge baseline, fraction of H
    amp: f32,                          // amplitude, fraction of H
    waves: &'static [(f32, f32, f32)], // (cycles across width, phase, weight)
    crest: &'static str,               // colour at the ridge
    deep: &'static str,                // colour far below the ridge
    fall: f32,                         // gradient depth, fraction of H
    rim: f32,                          // rim highlight strength
}

struct Preset {
    sky_top: &'static str,
    sky_bottom: &'static str,
    sun: Option<(f32, f32, f32, &'static str)>, // x, y, radius (fractions), glow colour
    shadow: f32,
    layers: Vec<Layer>,
}

fn ridge(l: &Layer, x: f32) -> f32 {
    let u = x / W as f32;
    let mut s = 0.0;
    let mut wsum = 0.0;
    for &(f, p, w) in l.waves {
        s += (u * f * std::f32::consts::TAU + p).sin() * w;
        wsum += w;
    }
    (l.base_y - l.amp * s / wsum) * H as f32
}

fn preset(name: &str) -> Preset {
    match name {
        // daylight rolling hills: pale sky, greens fading with distance
        "meadow" => Preset {
            sky_top: "5f9fd6",
            sky_bottom: "e4f1df",
            sun: Some((0.72, 0.18, 0.22, "fff6d8")),
            shadow: 0.22,
            layers: vec![
                Layer { base_y: 0.50, amp: 0.07, waves: &[(1.3, 0.4, 1.0), (2.7, 1.9, 0.5), (5.1, 0.2, 0.18)], crest: "c4e3b4", deep: "86b98c", fall: 0.30, rim: 0.25 },
                Layer { base_y: 0.62, amp: 0.08, waves: &[(1.1, 2.6, 1.0), (2.3, 0.7, 0.55), (4.6, 3.1, 0.2)], crest: "9fd47c", deep: "4f9a5a", fall: 0.30, rim: 0.30 },
                Layer { base_y: 0.75, amp: 0.08, waves: &[(0.9, 4.2, 1.0), (2.1, 1.3, 0.5), (3.9, 5.0, 0.22)], crest: "7cc957", deep: "2f7a3a", fall: 0.28, rim: 0.35 },
                Layer { base_y: 0.88, amp: 0.07, waves: &[(0.8, 1.0, 1.0), (1.9, 3.7, 0.45), (3.4, 2.2, 0.25)], crest: "5fb243", deep: "1d5a2a", fall: 0.22, rim: 0.40 },
            ],
        },
        // dark evergreen waves, same look as aurora/ocean-night
        "fern" => Preset {
            sky_top: "030807",
            sky_bottom: "0b1a12",
            sun: None,
            shadow: 0.55,
            layers: vec![
                Layer { base_y: 0.40, amp: 0.07, waves: &[(1.6, 0.3, 1.0), (3.2, 2.1, 0.45), (5.8, 4.0, 0.15)], crest: "2f6b3f", deep: "0c2416", fall: 0.30, rim: 0.45 },
                Layer { base_y: 0.56, amp: 0.09, waves: &[(1.3, 3.3, 1.0), (2.9, 0.9, 0.6), (5.0, 2.0, 0.2)], crest: "4a9a4f", deep: "113a20", fall: 0.30, rim: 0.55 },
                Layer { base_y: 0.72, amp: 0.09, waves: &[(1.0, 5.1, 1.0), (2.4, 1.6, 0.5), (4.3, 0.4, 0.2)], crest: "7cc96a", deep: "1a5a2c", fall: 0.30, rim: 0.65 },
                Layer { base_y: 0.88, amp: 0.07, waves: &[(0.8, 2.2, 1.0), (2.0, 4.4, 0.4), (3.6, 1.1, 0.2)], crest: "b3e89a", deep: "2d7a3c", fall: 0.25, rim: 0.75 },
            ],
        },
        // golden-hour valley: warm sky over olive and emerald hills
        "valley" => Preset {
            sky_top: "2a4a5e",
            sky_bottom: "f2c98a",
            sun: Some((0.30, 0.38, 0.30, "ffd9a0")),
            shadow: 0.35,
            layers: vec![
                Layer { base_y: 0.52, amp: 0.06, waves: &[(1.4, 1.2, 1.0), (3.0, 0.1, 0.5), (5.5, 2.4, 0.15)], crest: "a8b47a", deep: "5f7a52", fall: 0.30, rim: 0.30 },
                Layer { base_y: 0.64, amp: 0.08, waves: &[(1.2, 4.0, 1.0), (2.6, 2.2, 0.55), (4.8, 0.9, 0.2)], crest: "86a85a", deep: "3a6440", fall: 0.30, rim: 0.40 },
                Layer { base_y: 0.77, amp: 0.08, waves: &[(0.9, 0.6, 1.0), (2.2, 3.0, 0.5), (4.0, 1.7, 0.2)], crest: "5f9a48", deep: "234d2c", fall: 0.28, rim: 0.45 },
                Layer { base_y: 0.90, amp: 0.06, waves: &[(0.7, 2.9, 1.0), (1.8, 0.4, 0.45), (3.3, 4.6, 0.25)], crest: "3f7d3a", deep: "12301b", fall: 0.22, rim: 0.50 },
            ],
        },
        _ => panic!("unknown preset {}", name),
    }
}

fn main() {
    let name = std::env::args().nth(1).expect("usage: wavegen <meadow|fern|valley>");
    let p = preset(&name);
    let sky_top = hex(p.sky_top);
    let sky_bot = hex(p.sky_bottom);
    let ridges: Vec<Vec<f32>> = p.layers.iter().map(|l| (0..W).map(|x| ridge(l, x as f32)).collect()).collect();
    let crest: Vec<C> = p.layers.iter().map(|l| hex(l.crest)).collect();
    let deep: Vec<C> = p.layers.iter().map(|l| hex(l.deep)).collect();
    let shadow_len = 0.06 * H as f32;
    let rim_px = 0.004 * H as f32;
    let mut rng: u32 = 0x9e3779b9;

    let mut out = Vec::with_capacity(W * H * 3 + 32);
    write!(out, "P6\n{W} {H}\n255\n").unwrap();
    for y in 0..H {
        let yf = y as f32 + 0.5;
        for x in 0..W {
            let xf = x as f32;
            // sky
            let mut c = mix(sky_top, sky_bot, smooth(yf / (H as f32 * 0.75)));
            if let Some((sx, sy, r, g)) = p.sun {
                let dx = xf / W as f32 - sx;
                let dy = (yf / H as f32 - sy) * (H as f32 / W as f32);
                let d = (dx * dx + dy * dy).sqrt() / r;
                c = mix(c, hex(g), (-d * d * 2.5).exp() * 0.75);
            }
            // layers back to front, anti-aliased edge
            for (i, l) in p.layers.iter().enumerate() {
                let h = ridges[i][x];
                let d = yf - h;
                let cover = (d + 0.5).clamp(0.0, 1.0);
                if cover <= 0.0 {
                    continue;
                }
                let mut lc = mix(crest[i], deep[i], smooth(d / (l.fall * H as f32)));
                let rim = (-d.max(0.0) / rim_px).exp() * l.rim;
                lc = mix(lc, [1.0, 1.0, 1.0], rim * 0.6);
                // shadow cast up onto this layer by the next one in front
                if let Some(next) = ridges.get(i + 1) {
                    let gap = next[x] - yf;
                    if gap > 0.0 {
                        let s = (-gap / shadow_len).exp() * p.shadow;
                        lc = [lc[0] * (1.0 - s), lc[1] * (1.0 - s), lc[2] * (1.0 - s)];
                    }
                }
                c = mix(c, lc, cover);
            }
            // vignette + dither against banding
            let vx = xf / W as f32 - 0.5;
            let vy = yf / H as f32 - 0.5;
            let v = 1.0 - (vx * vx + vy * vy) * 0.35;
            for k in 0..3 {
                rng ^= rng << 13;
                rng ^= rng >> 17;
                rng ^= rng << 5;
                let n = (rng as f32 / u32::MAX as f32 - 0.5) * 1.5;
                out.push((c[k] * v * 255.0 + n).round().clamp(0.0, 255.0) as u8);
            }
        }
    }
    std::io::stdout().write_all(&out).unwrap();
}
