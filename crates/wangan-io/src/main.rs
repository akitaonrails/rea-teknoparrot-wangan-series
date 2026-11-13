//! wangan-io: the JVS I/O board for Wangan Midnight Maximum Tune 5–6RR,
//! run by the launcher inside the game's Wine session.
//!
//! ```text
//! wangan-io.exe --block <file> [--log <file>]
//! ```
//!
//! `--block` is the 64-byte input block the launcher writes
//! (`wangan_jvs::shared`); the board answers the game's JVS packets from it
//! on `\\.\pipe\teknoparrot_jvs` and keeps the "addressed" flag in the
//! `TeknoParrot_JvsState` section that OpenParrot reads. It exits when the
//! launcher sets the block's quit byte.

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
}

#[cfg(windows)]
fn parse_args() -> Result<Args, String> {
    let mut block = None;
    let mut log = None;
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--block" => block = it.next(),
            "--log" => log = it.next(),
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(Args {
        block: block.ok_or("--block is required")?,
        log,
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
