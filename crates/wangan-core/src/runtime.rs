//! The private runtime folder each game runs from.
//!
//! The player's game folder is read-only (it may be on a network share).
//! The runtime holds links to the big read-only files, copies of the few
//! files the game or its loader writes, empty save folders, and the
//! launcher's components (`rea_launcher::linkfarm`: on Windows a linked
//! folder is a directory junction and a linked file a copy kept in step).
//! [`plan`] decides from a listing of the game folder by each route's
//! [`Policy`]; `linkfarm::apply` carries it out without ever overwriting a
//! file the game wrote (saves, backups): only launcher-owned files are
//! refreshed, and a real file where a link goes is kept. Every rule here
//! was seen to matter in docs/RUNTIME_NOTES.md.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use rea_launcher::linkfarm::{self, Item, Op, Place, Policy};

use crate::catalog::{Game, GameId, Host, Route};

/// Files below this size in an ES3B folder's top level are copied: the
/// game rewrites its backup files (`F*/G*/BK_*`) there.
const SMALL: u64 = 8 * 1024 * 1024;

/// Where the launcher's own components live (one folder per route).
#[derive(Debug, Clone)]
pub struct Components {
    /// OpenParrot64.dll, OpenParrotLoader64.exe, iDmacDrv64.dll, bngrw.dll
    pub openparrot: PathBuf,
    /// The same files built from valkyrieasyraf/OpenParrot_WMMT with
    /// `patches/openparrot-wmmt` (the only build with a working 6RR module).
    pub openparrot_6rr: PathBuf,
    /// d3d11.dll, dxgi.dll (x64)
    pub dxvk: PathBuf,
    /// wangan-io.exe
    pub io_board: PathBuf,
    /// libwal_3dxp.so, libopenal.so, gamecontrollerdb.txt, tmp/
    pub wal: PathBuf,
    /// `app/` (the extracted pcsx2x6 AppImage, `app/AppRun`) and
    /// `NM00005/boot.elf` (Proverb).
    pub pcsx2x6: PathBuf,
}

impl Components {
    /// The OpenParrot build `game` runs with.
    pub fn openparrot_for(&self, game: &Game) -> &Path {
        if game.id == GameId::Mt6rr {
            &self.openparrot_6rr
        } else {
            &self.openparrot
        }
    }
}

/// An ES3B dump: what the game and OpenParrot write is copied or made
/// empty, the rest linked. Never brought in: leftovers in the owner's dumps
/// (dgVoodoo DLLs, a replaced card-reader DLL, scripts, NTFS metadata) and
/// files the launcher writes itself.
struct Es3b;

impl Policy for Es3b {
    fn place(&self, rel: &str, it: &Item) -> Place {
        let top = !rel.contains('/');
        let skip = matches!(
            rel,
            "D3D8.dll"
                | "D3D9.dll"
                | "DDraw.dll"
                | "D3DImm.dll"
                | "bngrw.dll"
                | "bngrw.dll.bak"
                | "[SYSTEM]"
                | "data_jp_ori"
                | "teknoparrot.ini"
                | "card.ini"
                | "d3d11.dll"
                | "dxgi.dll"
        ) || rel.ends_with(".ps1")
            || rel.ends_with(".jpg");
        match (rel, it.dir) {
            _ if top && skip => Place::Skip,
            // save folders OpenParrot redirects the game to: start empty
            ("TP" | "TeknoParrot", true) => Place::Empty,
            ("AMCUS" | "html", true) => Place::CopyOnce,
            ("data_jp", true) => Place::Open,
            // OpenParrot's WMMT6 module writes certificates here
            ("data_jp/network", true) => Place::CopyOnce,
            (_, false) if top && it.size < SMALL => Place::CopyOnce,
            _ => Place::Link,
        }
    }
}

/// An N2 dump: everything linked but the loader's files, the config the
/// launcher writes, and `main` (a launcher-owned copy: the share may not
/// keep the execute bit).
struct N2;

impl Policy for N2 {
    fn place(&self, rel: &str, _: &Item) -> Place {
        match rel {
            "libso" | "tmp" | "config.toml" | "keyconfig.toml" | "plugins" | "main" => Place::Skip,
            _ => Place::Link,
        }
    }
}

/// The runtime `rt` for `game` (its folder `src`) on `host`; `ls` lists a
/// folder ([`linkfarm::list`], or a fake in tests). Windows runs the games
/// on its own Direct3D 11, so DXVK goes in on Linux only; the N2 games and
/// Wangan R have no Windows runtime.
pub fn plan(
    game: &Game,
    src: &Path,
    rt: &Path,
    c: &Components,
    host: Host,
    ls: &dyn Fn(&Path) -> io::Result<Vec<Item>>,
) -> io::Result<Vec<Op>> {
    Ok(match (game.route, host) {
        (Route::Es3b, _) => {
            let mut ops = linkfarm::plan(src, rt, &Es3b, ls)?;
            ops.extend(es3b_components(rt, c, c.openparrot_for(game), host));
            ops
        }
        (Route::NativeN2, Host::Linux) => {
            let mut ops = linkfarm::plan(src, rt, &N2, ls)?;
            ops.extend(n2_components(src, rt, c));
            ops
        }
        (Route::System246, Host::Linux) => plan_sys246(src, rt, c),
        _ => vec![],
    })
}

/// Wangan R: the emulator linked in, the game folder pcsx2x6 expects
/// (`game/NM00005/`: the dump's CHD under the id's name, Proverb's
/// `boot.elf`), and its data folder (`config/PCSX2x6/`, the
/// `XDG_CONFIG_HOME` it runs with). The BIOS and the dongle card are put in
/// by the session (they come from the board files and the dump's zip).
fn plan_sys246(src: &Path, rt: &Path, c: &Components) -> Vec<Op> {
    use crate::sys246::GAME_ID;
    let game = rt.join("game").join(GAME_ID);
    let data = crate::sys246::data_dir(rt);
    vec![
        Op::Dir(rt.to_path_buf()),
        Op::Link {
            target: c.pcsx2x6.join("app"),
            at: rt.join("pcsx2x6"),
        },
        Op::Dir(game.clone()),
        Op::Link {
            target: src.join(crate::sys246::CHD),
            at: game.join(format!("{GAME_ID}.chd")),
        },
        Op::Refresh {
            from: c.pcsx2x6.join(GAME_ID).join("boot.elf"),
            to: game.join("boot.elf"),
            exec: false,
        },
        Op::Dir(data.join("bios")),
        Op::Dir(data.join("memcards")),
        Op::Dir(data.join("inis")),
    ]
}

/// The launcher's files in an ES3B runtime: OpenParrot, DXVK (Linux) and
/// the I/O board.
fn es3b_components(rt: &Path, c: &Components, op: &Path, host: Host) -> Vec<Op> {
    let refresh = |from: PathBuf, name: &str| Op::Refresh {
        from,
        to: rt.join(name),
        exec: false,
    };
    let mut ops: Vec<Op> = [
        "OpenParrot64.dll",
        "OpenParrotLoader64.exe",
        "iDmacDrv64.dll",
        "bngrw.dll",
    ]
    .into_iter()
    .map(|f| refresh(op.join(f), f))
    .collect();
    if host == Host::Linux {
        ops.extend(["d3d11.dll", "dxgi.dll"].map(|f| refresh(c.dxvk.join(f), f)));
    }
    ops.push(refresh(c.io_board.join("wangan-io.exe"), "wangan-io.exe"));
    ops
}

/// The launcher's files in an N2 runtime: the game's program, the loader
/// and its temporary folder, the folders the game writes its data to.
fn n2_components(src: &Path, rt: &Path, c: &Components) -> Vec<Op> {
    let mut ops = vec![
        Op::Refresh {
            from: src.join("main"),
            to: rt.join("main"),
            exec: true,
        },
        Op::Dir(rt.join("libso")),
    ];
    for f in ["libwal_3dxp.so", "libopenal.so", "gamecontrollerdb.txt"] {
        ops.push(Op::Refresh {
            from: c.wal.join(f),
            to: rt.join(f),
            exec: false,
        });
    }
    ops.push(Op::TreeOnce {
        from: c.wal.join("tmp"),
        to: rt.join("tmp"),
    });
    for d in ["etc", "joinstar", "maxicoin", "ranking", "target"] {
        ops.push(Op::Dir(rt.join("tmp/data").join(d)));
    }
    ops.push(Op::CopyOnce {
        from: src.join("data/sound/bgm/maxi3/sys_04.wav"),
        to: rt.join("tmp/sys_04.wav"),
    });
    ops
}

/// The `libso/` links of an N2 runtime: every real ELF library linked under
/// its own name and its SONAME. The dumps lost their symlinks (0-byte files
/// or text stubs) and those are skipped; the dump's `libstdc++`/`libz` are
/// left out so the system's 32-bit ones load, as the loader's `start.sh`
/// does.
pub fn plan_libso(src_libso: &Path, rt_libso: &Path) -> io::Result<Vec<Op>> {
    let mut ops = Vec::new();
    let mut names: Vec<_> = fs::read_dir(src_libso)?
        .filter_map(Result::ok)
        .map(|e| e.file_name())
        .collect();
    names.sort();
    let mut taken = std::collections::HashSet::new();
    for n in names {
        let name = n.to_string_lossy().into_owned();
        if name.starts_with("libstdc++.so") || name.starts_with("libz.so") {
            continue;
        }
        let path = src_libso.join(&name);
        let Ok(bytes) = fs::read(&path) else { continue };
        if !bytes.starts_with(b"\x7FELF") {
            continue;
        }
        if taken.insert(name.clone()) {
            ops.push(Op::Link {
                target: path.clone(),
                at: rt_libso.join(&name),
            });
        }
        if let Some(so) = elf32_soname(&bytes)
            && so != name
            && taken.insert(so.clone())
        {
            ops.push(Op::Link {
                target: path,
                at: rt_libso.join(so),
            });
        }
    }
    Ok(ops)
}

/// DT_SONAME of a 32-bit little-endian ELF shared object.
pub fn elf32_soname(b: &[u8]) -> Option<String> {
    let u16le = |o: usize| b.get(o..o + 2).map(|s| u16::from_le_bytes([s[0], s[1]]));
    let u32le = |o: usize| {
        b.get(o..o + 4)
            .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    };
    if b.get(4) != Some(&1) || b.get(5) != Some(&1) {
        return None; // not ELFCLASS32 little-endian
    }
    let phoff = u32le(28)? as usize;
    let phentsize = u16le(42)? as usize;
    let phnum = u16le(44)? as usize;
    let mut loads = Vec::new();
    let mut dynamic = None;
    for i in 0..phnum {
        let p = phoff + i * phentsize;
        let (ty, off, vaddr, filesz) = (u32le(p)?, u32le(p + 4)?, u32le(p + 8)?, u32le(p + 16)?);
        match ty {
            1 => loads.push((vaddr, off, filesz)),
            2 => dynamic = Some((off as usize, filesz as usize)),
            _ => {}
        }
    }
    let (doff, dsz) = dynamic?;
    let (mut strtab, mut soname) = (None, None);
    for i in 0..dsz / 8 {
        let tag = u32le(doff + i * 8)?;
        let val = u32le(doff + i * 8 + 4)?;
        match tag {
            0 => break,
            5 => strtab = Some(val),
            14 => soname = Some(val),
            _ => {}
        }
    }
    let strtab = strtab?;
    let file_off = loads
        .iter()
        .find(|(va, _, sz)| strtab >= *va && strtab < va + sz)
        .map(|(va, off, _)| (strtab - va + off) as usize)?;
    let start = file_off + soname? as usize;
    let end = start + b.get(start..)?.iter().position(|&c| c == 0)?;
    String::from_utf8(b[start..end].to_vec()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::game;
    use rea_launcher::linkfarm::Occupied;

    fn comps(base: &Path) -> Components {
        Components {
            openparrot: base.join("op"),
            openparrot_6rr: base.join("op6rr"),
            dxvk: base.join("dxvk"),
            io_board: base.join("io"),
            wal: base.join("wal"),
            pcsx2x6: base.join("pcsx2x6"),
        }
    }

    fn item(name: &str, dir: bool, size: u64) -> Item {
        Item {
            name: name.into(),
            dir,
            size,
        }
    }

    /// A listing per folder, relative to `src`.
    fn fake<'a>(
        src: &'a Path,
        tree: &'a [(&'a str, Vec<Item>)],
    ) -> impl Fn(&Path) -> io::Result<Vec<Item>> + 'a {
        move |dir| {
            let rel = dir.strip_prefix(src).unwrap().to_string_lossy();
            Ok(tree
                .iter()
                .find(|(d, _)| *d == rel)
                .map(|(_, items)| items.clone())
                .unwrap_or_default())
        }
    }

    #[test]
    fn wangan_r_gets_the_emulator_the_chd_and_proverb_under_the_game_id() {
        let (src, rt, c) = (Path::new("/lib/wmr"), Path::new("/rt"), Path::new("/c"));
        let none = |_: &Path| -> io::Result<Vec<Item>> { Ok(vec![]) };
        let wr = game(GameId::WanganR);
        let ops = plan(wr, src, rt, &comps(c), Host::Linux, &none).unwrap();
        let game_dir = rt.join("game").join("NM00005");
        assert!(ops.contains(&Op::Link {
            target: c.join("pcsx2x6").join("app"),
            at: rt.join("pcsx2x6"),
        }));
        assert!(ops.contains(&Op::Link {
            target: src.join("wanganmr/wmr1-a.chd"),
            at: game_dir.join("NM00005.chd"),
        }));
        assert!(ops.contains(&Op::Refresh {
            from: c.join("pcsx2x6").join("NM00005").join("boot.elf"),
            to: game_dir.join("boot.elf"),
            exec: false,
        }));
        assert!(ops.contains(&Op::Dir(rt.join("config").join("PCSX2x6").join("memcards"))));
        let windows = plan(wr, src, rt, &comps(c), Host::Windows, &none).unwrap();
        assert!(windows.is_empty());
    }

    #[test]
    fn es3b_plan_follows_the_runtime_rules() {
        let src = Path::new("/games/MT6");
        let rt = Path::new("/rt/mt6");
        let tree = [
            (
                "",
                vec![
                    item("wmn6r.exe", false, 33_000_000),
                    item("Fsetting.lua.gz", false, 408),
                    item("D3D9.dll", false, 400_000),
                    item("bngrw.dll", false, 342_000),
                    item("init.ps1", false, 2_700),
                    item("TP", true, 0),
                    item("AMCUS", true, 0),
                    item("data", true, 0),
                    item("data_jp", true, 0),
                    item("[SYSTEM]", true, 0),
                ],
            ),
            (
                "data_jp",
                vec![
                    item("movie", true, 0),
                    item("network", true, 0),
                    item("big.bin", false, 64 << 20),
                    item("small.bin", false, 10),
                ],
            ),
        ];
        let c = comps(Path::new("/c"));
        let ops = plan(
            game(GameId::Mt6),
            src,
            rt,
            &c,
            Host::Linux,
            &fake(src, &tree),
        )
        .unwrap();
        assert!(ops.contains(&Op::Link {
            target: src.join("wmn6r.exe"),
            at: rt.join("wmn6r.exe")
        }));
        assert!(ops.contains(&Op::CopyOnce {
            from: src.join("Fsetting.lua.gz"),
            to: rt.join("Fsetting.lua.gz")
        }));
        assert!(ops.contains(&Op::Dir(rt.join("TP"))));
        assert!(ops.contains(&Op::TreeOnce {
            from: src.join("AMCUS"),
            to: rt.join("AMCUS")
        }));
        assert!(ops.contains(&Op::Dir(rt.join("data_jp"))));
        assert!(ops.contains(&Op::Link {
            target: src.join("data_jp/movie"),
            at: rt.join("data_jp/movie")
        }));
        assert!(ops.contains(&Op::TreeOnce {
            from: src.join("data_jp/network"),
            to: rt.join("data_jp/network")
        }));
        // only the top level's small files are the game's backups
        assert!(ops.contains(&Op::Link {
            target: src.join("data_jp/small.bin"),
            at: rt.join("data_jp/small.bin")
        }));
        assert!(ops.contains(&Op::Refresh {
            from: PathBuf::from("/c/op/bngrw.dll"),
            to: rt.join("bngrw.dll"),
            exec: false
        }));
        for gone in ["D3D9.dll", "init.ps1", "[SYSTEM]"] {
            assert!(
                !ops.iter().any(|o| format!("{o:?}").contains(gone)),
                "{gone} must be left out"
            );
        }
        // the dump's own bngrw.dll never comes in, only ours
        assert!(!ops.contains(&Op::CopyOnce {
            from: src.join("bngrw.dll"),
            to: rt.join("bngrw.dll")
        }));
    }

    #[test]
    fn six_rr_takes_the_fork_build_and_the_others_upstream() {
        let (src, rt) = (Path::new("/g"), Path::new("/rt"));
        let tree = [("", vec![item("wmn6r.exe", false, 33_000_000)])];
        let c = comps(Path::new("/c"));
        let dll = |id| {
            let ops = plan(game(id), src, rt, &c, Host::Linux, &fake(src, &tree)).unwrap();
            ops.iter().find_map(|o| match o {
                Op::Refresh { from, to, .. } if to.ends_with("OpenParrot64.dll") => {
                    Some(from.clone())
                }
                _ => None,
            })
        };
        assert_eq!(
            dll(GameId::Mt6rr),
            Some(PathBuf::from("/c/op6rr/OpenParrot64.dll"))
        );
        assert_eq!(
            dll(GameId::Mt6r),
            Some(PathBuf::from("/c/op/OpenParrot64.dll"))
        );
    }

    #[test]
    fn apply_keeps_game_written_files_and_refreshes_ours() {
        let t = tempfile::tempdir().unwrap();
        let src = t.path().join("src");
        let rt = t.path().join("rt");
        fs::create_dir_all(src.join("TP")).unwrap();
        fs::write(src.join("FBK.bin"), b"dump").unwrap();
        fs::write(src.join("big.exe"), b"exe").unwrap();
        let c = t.path().join("c");
        for d in ["op", "dxvk", "io"] {
            fs::create_dir_all(c.join(d)).unwrap();
        }
        for f in [
            "OpenParrot64.dll",
            "OpenParrotLoader64.exe",
            "iDmacDrv64.dll",
            "bngrw.dll",
        ] {
            fs::write(c.join("op").join(f), b"v1").unwrap();
        }
        for f in ["d3d11.dll", "dxgi.dll"] {
            fs::write(c.join("dxvk").join(f), b"v1").unwrap();
        }
        fs::write(c.join("io/wangan-io.exe"), b"v1").unwrap();
        let comps = comps(&c);
        let mt5 = game(GameId::Mt5);
        let ops = plan(mt5, &src, &rt, &comps, Host::current(), &linkfarm::list).unwrap();
        linkfarm::apply(&ops, Occupied::Keep).unwrap();
        // the game writes its backup file; a later launch must keep it
        fs::write(rt.join("FBK.bin"), b"player save").unwrap();
        fs::write(c.join("op/OpenParrot64.dll"), b"v2").unwrap();
        linkfarm::apply(&ops, Occupied::Keep).unwrap();
        assert_eq!(fs::read(rt.join("FBK.bin")).unwrap(), b"player save");
        assert_eq!(fs::read(rt.join("OpenParrot64.dll")).unwrap(), b"v2");
        assert!(rt.join("TP").is_dir());
        // and the source was never written
        assert_eq!(fs::read(src.join("FBK.bin")).unwrap(), b"dump");
    }

    #[test]
    fn windows_runtime_leaves_dxvk_out_and_has_no_n2_games() {
        let (src, rt) = (Path::new("/g"), Path::new("/rt"));
        let tree = [("", vec![item("wmn5r.exe", false, 33_000_000)])];
        let ls = fake(src, &tree);
        let c = comps(Path::new("/c"));
        let dxvk = |ops: &[Op]| {
            ops.iter()
                .filter(|o| matches!(o, Op::Refresh { from, .. } if from.starts_with("/c/dxvk")))
                .count()
        };
        let mt5 = game(GameId::Mt5);
        let linux = plan(mt5, src, rt, &c, Host::Linux, &ls).unwrap();
        let windows = plan(mt5, src, rt, &c, Host::Windows, &ls).unwrap();
        assert_eq!(dxvk(&linux), 2);
        assert_eq!(dxvk(&windows), 0);
        // everything else is the same runtime
        assert_eq!(linux.len() - 2, windows.len());
        assert!(windows.contains(&Op::Refresh {
            from: PathBuf::from("/c/io/wangan-io.exe"),
            to: rt.join("wangan-io.exe"),
            exec: false
        }));
        let n2 = [("", vec![item("main", false, 9_000_000)])];
        let ls = fake(src, &n2);
        let mt3 = game(GameId::Mt3);
        assert!(
            plan(mt3, src, rt, &c, Host::Windows, &ls)
                .unwrap()
                .is_empty()
        );
        assert!(!plan(mt3, src, rt, &c, Host::Linux, &ls).unwrap().is_empty());
    }

    /// Links in the runtime: the folder is reachable through it, the source
    /// is never written, a second apply changes nothing, and a real folder
    /// the game made in a link's place is kept. Junctions and copies on
    /// Windows, symlinks elsewhere.
    #[test]
    fn links_reach_the_game_folder_and_survive_a_second_apply() {
        let t = tempfile::tempdir().unwrap();
        let src = t.path().join("game");
        let rt = t.path().join("rt");
        fs::create_dir_all(src.join("data")).unwrap();
        fs::write(src.join("data/a.bin"), b"data").unwrap();
        fs::write(src.join("big.exe"), b"exe").unwrap();
        fs::create_dir_all(src.join("movie")).unwrap();
        let ops = vec![
            Op::Dir(rt.clone()),
            Op::Link {
                target: src.join("data"),
                at: rt.join("data"),
            },
            Op::Link {
                target: src.join("big.exe"),
                at: rt.join("big.exe"),
            },
        ];
        linkfarm::apply(&ops, Occupied::Keep).unwrap();
        linkfarm::apply(&ops, Occupied::Keep).unwrap();
        assert_eq!(fs::read(rt.join("data/a.bin")).unwrap(), b"data");
        assert_eq!(fs::read(rt.join("big.exe")).unwrap(), b"exe");
        // a link pointing elsewhere is moved, the folder it pointed to untouched
        let moved = vec![Op::Link {
            target: src.join("movie"),
            at: rt.join("data"),
        }];
        linkfarm::apply(&moved, Occupied::Keep).unwrap();
        assert!(rt.join("data").read_dir().unwrap().next().is_none());
        assert_eq!(fs::read(src.join("data/a.bin")).unwrap(), b"data");
        // a real folder in a link's place is kept
        fs::create_dir_all(rt.join("TP")).unwrap();
        fs::write(rt.join("TP/save"), b"s").unwrap();
        let tp = [Op::Link {
            target: src.join("data"),
            at: rt.join("TP"),
        }];
        linkfarm::apply(&tp, Occupied::Keep).unwrap();
        assert_eq!(fs::read(rt.join("TP/save")).unwrap(), b"s");
    }

    #[test]
    fn soname_of_a_synthetic_elf32() {
        // minimal ELF32: one PT_LOAD at 0, one PT_DYNAMIC, strtab with "libx.so.1"
        let mut b = vec![0u8; 0x200];
        b[..4].copy_from_slice(b"\x7FELF");
        b[4] = 1;
        b[5] = 1;
        b[28..32].copy_from_slice(&52u32.to_le_bytes()); // phoff
        b[42..44].copy_from_slice(&32u16.to_le_bytes()); // phentsize
        b[44..46].copy_from_slice(&2u16.to_le_bytes()); // phnum
        let ph = |b: &mut Vec<u8>, at: usize, ty: u32, off: u32, va: u32, sz: u32| {
            b[at..at + 4].copy_from_slice(&ty.to_le_bytes());
            b[at + 4..at + 8].copy_from_slice(&off.to_le_bytes());
            b[at + 8..at + 12].copy_from_slice(&va.to_le_bytes());
            b[at + 16..at + 20].copy_from_slice(&sz.to_le_bytes());
        };
        ph(&mut b, 52, 1, 0, 0x1000, 0x200);
        ph(&mut b, 84, 2, 0x100, 0x1100, 24);
        // dynamic: DT_STRTAB=0x1180, DT_SONAME=1, DT_NULL
        b[0x100..0x104].copy_from_slice(&5u32.to_le_bytes());
        b[0x104..0x108].copy_from_slice(&0x1180u32.to_le_bytes());
        b[0x108..0x10C].copy_from_slice(&14u32.to_le_bytes());
        b[0x10C..0x110].copy_from_slice(&1u32.to_le_bytes());
        b[0x181..0x18A].copy_from_slice(b"libx.so.1");
        assert_eq!(elf32_soname(&b).as_deref(), Some("libx.so.1"));
        assert_eq!(elf32_soname(b"\x7FELF\x02\x01"), None);
    }

    /// The real MT3DX+ libso (`WANGAN_GAMES_DIR=… cargo test -- --ignored`).
    #[test]
    #[ignore]
    fn real_mt3dxp_libso_gets_its_boost_sonames() {
        let Ok(root) = std::env::var("WANGAN_GAMES_DIR") else {
            return;
        };
        let dir = Path::new(&root)
            .join("Wangan Midnight Maximum Tune 3DX+ (EXP)(2010)[Namco N2][TP]/libso");
        let ops = plan_libso(&dir, Path::new("/rt/libso")).unwrap();
        let names: Vec<String> = ops
            .iter()
            .filter_map(|o| match o {
                Op::Link { at, .. } => Some(at.file_name().unwrap().to_string_lossy().into_owned()),
                _ => None,
            })
            .collect();
        assert!(
            names.contains(&"libboost_system-gcc40-mt-1_36.so.1.36.0".to_string()),
            "{names:?}"
        );
        assert!(!names.iter().any(|n| n.starts_with("libstdc++")));
    }
}
