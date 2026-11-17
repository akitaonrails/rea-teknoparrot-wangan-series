//! wangan-io: the JVS I/O board for Wangan Midnight Maximum Tune 5–6RR,
//! run by the launcher inside the game's Wine session.
//!
//! ```text
//! wangan-io.exe --block <file> [--log <file>] [--chihiro]
//! ```
//!
//! `--block` is the 64-byte input block the launcher writes
//! (`wangan_jvs::shared`); the board answers the game's JVS packets from it
//! on `\\.\pipe\teknoparrot_jvs` and keeps the "addressed" flag in the
//! `TeknoParrot_JvsState` section that OpenParrot reads. It exits when the
//! launcher sets the block's quit byte. With `--chihiro` (Maximum Tune 1)
//! there is no pipe: the emulator holds the board and reads the controls
//! from `TeknoParrot_JvsState`, which this keeps filled (`wangan_jvs::chihiro`).

#[cfg(windows)]
mod win32;

#[cfg(windows)]
fn main() {
    std::process::exit(match run() {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("wangan-io: {e}");
            1
        }
    });
}

#[cfg(not(windows))]
fn main() {
    eprintln!("wangan-io runs under Windows or Wine only");
    std::process::exit(4);
}

#[cfg(windows)]
struct Args {
    block: String,
    log: Option<String>,
    chihiro: bool,
}

#[cfg(windows)]
fn parse_args() -> Result<Args, String> {
    let mut block = None;
    let mut log = None;
    let mut chihiro = false;
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--block" => block = it.next(),
            "--log" => log = it.next(),
            "--chihiro" => chihiro = true,
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(Args {
        block: block.ok_or("--block is required")?,
        log,
        chihiro,
    })
}

#[cfg(windows)]
fn run() -> Result<(), String> {
    use std::io::Write;
    use std::sync::{Arc, Mutex};
    use wangan_jvs::board::{Board, NAMCO_WMMT};
    use wangan_jvs::frame::Assembler;

    let args = parse_args()?;
    let log: Arc<Mutex<Option<std::fs::File>>> =
        Arc::new(Mutex::new(args.log.as_ref().and_then(|p| {
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(p)
                .ok()
        })));
    let say = {
        let log = log.clone();
        move |line: String| {
            if let Ok(mut g) = log.lock()
                && let Some(f) = g.as_mut()
            {
                let _ = writeln!(f, "{line}");
                let _ = f.flush();
            }
        }
    };

    let state = win32::JvsState::create().map_err(|e| format!("TeknoParrot_JvsState: {e}"))?;
    let block =
        Arc::new(win32::InputBlock::open(&args.block).map_err(|e| format!("{}: {e}", args.block))?);
    say(format!(
        "wangan-io {} ready, block {}",
        env!("CARGO_PKG_VERSION"),
        args.block
    ));

    // Quit watcher: the pipe read blocks, so a second thread ends the process.
    {
        let block = block.clone();
        let say = say.clone();
        std::thread::spawn(move || {
            loop {
                if block.snapshot().is_some_and(|s| s.quit) {
                    say("quit requested".into());
                    std::process::exit(0);
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        });
    }

    if args.chihiro {
        return chihiro(&state, &block, &say);
    }

    let mut board = Board::new(NAMCO_WMMT);
    let mut logged = 0usize;
    loop {
        let pipe =
            win32::Pipe::create(r"\\.\pipe\teknoparrot_jvs").map_err(|e| format!("pipe: {e}"))?;
        pipe.connect().map_err(|e| format!("connect: {e}"))?;
        say("game connected".into());
        let mut asm = Assembler::new();
        let mut buf = [0u8; 1024];
        loop {
            let n = match pipe.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            };
            for packet in asm.push(&buf[..n]) {
                let input = block.snapshot().unwrap_or_default();
                let out = board.reply(&packet, &input);
                if let Some(a) = out.addressed {
                    state.set_addressed(a);
                }
                if logged < 64 {
                    logged += 1;
                    say(format!(
                        "rx {:02X?} -> {}",
                        packet,
                        out.reply
                            .as_ref()
                            .map_or("none".into(), |r| format!("{r:02X?}"))
                    ));
                }
                if let Some(reply) = out.reply
                    && pipe.write(&reply).is_err()
                {
                    break;
                }
            }
        }
        say("game disconnected".into());
    }
}

/// Maximum Tune 1: the emulator has the board inside and polls
/// `TeknoParrot_JvsState`; keep it filled from the launcher's block
/// (`wangan_jvs::chihiro`). Runs until the quit watcher exits.
#[cfg(windows)]
fn chihiro(
    state: &win32::JvsState,
    block: &win32::InputBlock,
    say: &dyn Fn(String),
) -> Result<(), String> {
    use wangan_jvs::chihiro::{ANALOG, COIN, CONTROL, CoinSwitch, analog, control};
    say("chihiro mode: writing TeknoParrot_JvsState".into());
    let start = std::time::Instant::now();
    let mut coin = CoinSwitch::default();
    loop {
        let s = block.snapshot().unwrap_or_default();
        state.put_u32(CONTROL, control(&s));
        state.put_u32(ANALOG, u32::from_le_bytes(analog(&s)));
        let down = coin.down(s.coins[0], start.elapsed().as_millis() as u64);
        state.put_u32(COIN, down as u32);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}
