//! The input thread: it owns SDL, reads wheels and pads, turns them into
//! the cabinet's controls and, while a game runs, writes them into the
//! game's input block (the I/O board under Wine, or the N2 loader, reads
//! it). Between games it sends menu navigation to the window.

use std::fs::OpenOptions;
use std::os::unix::fs::FileExt;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use rea_launcher::pad::{Nav, PadNav};
use rea_launcher::wheel::{
    Controls as Read, DeviceId, Kind, PadSettings, WheelProfile, WheelSettings, read_pad,
};
use rea_launcher_native::policy::HapticPolicy;
use rea_launcher_native::sdl::{Devices, Sdl};
use wangan_core::input::{Button, Translator, default_pad_map};
use wangan_jvs::controls::{Cabinet, Controls};

const TICK: Duration = Duration::from_millis(4);

#[derive(Debug, Clone, PartialEq)]
pub struct DeviceInfo {
    pub name: String,
    pub kind: Kind,
    pub haptic: bool,
}

/// What the window shows.
#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    pub devices: Vec<DeviceInfo>,
    /// The device driving the car, by index into `devices`.
    pub driver: Option<usize>,
    pub controls: Controls,
    pub writing: bool,
    pub error: Option<String>,
}

pub enum Command {
    Attach { block: PathBuf, cabinet: Cabinet },
    Detach,
}

pub struct InputService {
    tx: Sender<Command>,
    pub nav: Receiver<Nav>,
    snapshot: Arc<Mutex<Snapshot>>,
    thread: Option<JoinHandle<()>>,
}

impl InputService {
    pub fn start() -> Self {
        let (tx, rx) = mpsc::channel();
        let (nav_tx, nav) = mpsc::channel();
        let snapshot = Arc::new(Mutex::new(Snapshot::default()));
        let shared = Arc::clone(&snapshot);
        let thread = std::thread::Builder::new()
            .name("input".into())
            .spawn(move || run(rx, nav_tx, shared))
            .expect("spawn input thread");
        Self {
            tx,
            nav,
            snapshot,
            thread: Some(thread),
        }
    }

    pub fn send(&self, c: Command) {
        let _ = self.tx.send(c);
    }

    pub fn snapshot(&self) -> Snapshot {
        self.snapshot.lock().map(|s| s.clone()).unwrap_or_default()
    }
}

impl Drop for InputService {
    fn drop(&mut self) {
        let (tx, _) = mpsc::channel();
        drop(std::mem::replace(&mut self.tx, tx));
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

struct Writer {
    file: std::fs::File,
    cabinet: Cabinet,
    seq: u32,
}

fn run(rx: Receiver<Command>, nav_tx: Sender<Nav>, shared: Arc<Mutex<Snapshot>>) {
    let sdl = match Sdl::init(&[]) {
        Ok(s) => s,
        Err(e) => {
            if let Ok(mut s) = shared.lock() {
                s.error = Some(format!("controllers unavailable: {e}"));
            }
            while rx.recv().is_ok() {}
            return;
        }
    };
    let mut devices = Devices::new(&sdl, HapticPolicy::default());
    let pad_settings = PadSettings::new(default_pad_map());
    let wheel_settings = WheelSettings::default();
    let mut profile: Option<(DeviceId, WheelProfile<Button>)> = None;
    let mut translator = Translator::default();
    let mut writer: Option<Writer> = None;
    let mut nav = PadNav::with_repeat(Duration::from_millis(400), Duration::from_millis(120));
    loop {
        loop {
            match rx.try_recv() {
                Ok(Command::Attach { block, cabinet }) => {
                    translator = Translator::default();
                    writer = OpenOptions::new()
                        .write(true)
                        .open(&block)
                        .ok()
                        .map(|file| Writer {
                            file,
                            cabinet,
                            seq: 0,
                        });
                }
                Ok(Command::Detach) => writer = None,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return,
            }
        }
        devices.pump();
        let list = devices.list();
        let wheel = list.iter().position(|d| d.kind == Kind::Wheel);
        let mut read = Read::default();
        if let Some(w) = wheel {
            let d = &list[w];
            let fresh = profile
                .as_ref()
                .is_none_or(|(id, _)| id.vid != d.id.vid || id.pid != d.id.pid);
            if fresh {
                profile = Some((
                    d.id.clone(),
                    WheelProfile::guess(
                        d.id.clone(),
                        &d.initial,
                        Some((Button::GearUp, Button::GearDown)),
                    ),
                ));
            }
            if let Some((_, p)) = &profile {
                read = p.read(&wheel_settings, &d.state, None);
            }
        }
        let mut pad_driver = None;
        for (i, d) in list.iter().enumerate() {
            if let Some(pad) = d.pad {
                read = read.merge(read_pad(&pad_settings, &pad, None));
                pad_driver.get_or_insert(i);
            }
        }
        let controls = translator.step(&read);
        if let Some(w) = writer.as_mut() {
            w.seq = w.seq.wrapping_add(1);
            let bytes = controls.to_snapshot(w.cabinet, [0; 4], w.seq).encode();
            // byte 57 (quit) is the session's; write the rest
            let _ = w.file.write_all_at(&bytes[..wangan_jvs::shared::QUIT], 0);
        } else {
            let bits = list
                .iter()
                .filter_map(|d| d.pad.map(|p| p.nav_bits()))
                .fold(0u16, |a, b| a | b);
            for n in nav.update_at(bits, Instant::now()) {
                let _ = nav_tx.send(n);
            }
        }
        if let Ok(mut s) = shared.lock() {
            *s = Snapshot {
                devices: list
                    .iter()
                    .map(|d| DeviceInfo {
                        name: d.id.name.clone(),
                        kind: d.kind,
                        haptic: d.haptic,
                    })
                    .collect(),
                driver: wheel.or(pad_driver),
                controls,
                writing: writer.is_some(),
                error: None,
            };
        }
        std::thread::sleep(TICK);
    }
}
