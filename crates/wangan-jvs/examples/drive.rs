//! Test driver: replays a timed control script into an input block file,
//! so a headless run can play the game without a pad.
//!
//! ```text
//! cargo run --example drive -- <block-file> <wmmt5|wmmt6rr> <script>
//! ```
//!
//! Script lines: `<seconds> key=value …`; the controls hold until the next
//! line. Keys: wheel, gas, brake (numbers), gear (0-6), view, intrude,
//! card, test, service, up, down, enter (0/1), coin (adds that many coins),
//! quit (sets the quit byte and ends).

use std::io::Write;
use std::time::{Duration, Instant};
use wangan_jvs::controls::{Cabinet, Controls};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() != 4 {
        eprintln!("usage: drive <block-file> <wmmt5|wmmt6rr> <script>");
        std::process::exit(2);
    }
    let cabinet = match a[2].as_str() {
        "wmmt5" => Cabinet::Wmmt5,
        "wmmt6rr" => Cabinet::Wmmt6RR,
        x => panic!("cabinet {x}"),
    };
    let script = std::fs::read_to_string(&a[3]).expect("script");
    let mut steps: Vec<(f64, Vec<(String, String)>)> = Vec::new();
    for line in script
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
    {
        let mut parts = line.split_whitespace();
        let t: f64 = parts.next().unwrap().parse().expect("time");
        let kv = parts
            .map(|p| {
                let (k, v) = p.split_once('=').unwrap_or((p, "1"));
                (k.to_string(), v.to_string())
            })
            .collect();
        steps.push((t, kv));
    }
    let start = Instant::now();
    let mut c = Controls::default();
    let mut coins = [0u16; 4];
    let mut seq = 0u32;
    let mut next = 0usize;
    let mut quit = false;
    loop {
        let now = start.elapsed().as_secs_f64();
        while next < steps.len() && steps[next].0 <= now {
            c = Controls {
                gear: c.gear,
                ..Controls::default()
            };
            for (k, v) in &steps[next].1 {
                let f = || v.parse::<f32>().expect("number");
                let b = || v != "0";
                match k.as_str() {
                    "wheel" => c.wheel = f(),
                    "gas" => c.gas = f(),
                    "brake" => c.brake = f(),
                    "gear" => c.gear = v.parse().expect("gear"),
                    "view" => c.view = b(),
                    "intrude" => c.intrude = b(),
                    "card" => c.card = b(),
                    "test" => c.test = b(),
                    "service" => c.service = b(),
                    "up" => c.menu_up = b(),
                    "down" => c.menu_down = b(),
                    "enter" => c.enter = b(),
                    "coin" => coins[0] += v.parse::<u16>().expect("coins"),
                    "quit" => quit = true,
                    x => panic!("unknown key {x}"),
                }
            }
            eprintln!("t={now:.1} {:?}", steps[next].1);
            next += 1;
        }
        seq = seq.wrapping_add(1);
        let mut s = c.to_snapshot(cabinet, coins, seq);
        s.quit = quit;
        let bytes = s.encode();
        // In place, not rename: the board keeps the file mapped.
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(&a[1])
            .expect("block");
        f.write_all(&bytes).expect("write");
        drop(f);
        if quit {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
