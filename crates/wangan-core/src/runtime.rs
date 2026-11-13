//! The private runtime folder each game runs from.
//!
//! The player's game folder is read-only (it may be on a network share).
//! The runtime holds symlinks to the big read-only files, copies of the few
//! files the game or its loader writes, empty save folders, and the
//! launcher's components. [`plan`] decides from a listing of the game
//! folder; [`apply`] carries it out without ever overwriting a file the
//! game wrote (saves, backups) — only launcher-owned files are refreshed.
//! Every rule here was seen to matter in docs/RUNTIME_NOTES.md.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::catalog::{Game, Route};

/// Files below this size in an ES3B folder's top level are copied: the
/// game rewrites its backup files (`F*/G*/BK_*`) there.
const SMALL: u64 = 8 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    Dir(PathBuf),
    /// Symlink `at` → `target`.
    Link {
        target: PathBuf,
        at: PathBuf,
    },
    /// Copy once; an existing file is the game's and is kept.
    CopyOnce {
        from: PathBuf,
        to: PathBuf,
    },
    /// Copy a folder tree once (folders the game writes into).
    TreeOnce {
        from: PathBuf,
        to: PathBuf,
    },
    /// Launcher-owned: replaced whenever the content differs.
    Refresh {
        from: PathBuf,
        to: PathBuf,
        exec: bool,
    },
}

/// One entry of the game folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub name: String,
    pub dir: bool,
    pub size: u64,
}

pub fn list(dir: &Path) -> io::Result<Vec<Item>> {
    let mut v = Vec::new();
    for e in fs::read_dir(dir)? {
        let e = e?;
        let m = fs::metadata(e.path())?;
        v.push(Item {
            name: e.file_name().to_string_lossy().into_owned(),
            dir: m.is_dir(),
            size: m.len(),
        });
    }
    v.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(v)
}

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
}

/// Names never brought into an ES3B runtime: leftovers in the owner's dumps
/// (dgVoodoo DLLs, a replaced card-reader DLL, scripts, NTFS metadata) and
/// files the launcher writes itself.
fn es3b_skip(name: &str) -> bool {
    matches!(
        name,
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
    ) || name.ends_with(".ps1")
        || name.ends_with(".jpg")
}

pub fn plan(
    game: &Game,
    src: &Path,
    items: &[Item],
    network_items: &[Item],
    rt: &Path,
    c: &Components,
) -> Vec<Op> {
    match game.route {
        Route::Es3b => {
            let op = if game.id == crate::catalog::GameId::Mt6rr {
                &c.openparrot_6rr
            } else {
                &c.openparrot
            };
            plan_es3b(src, items, network_items, rt, c, op)
        }
        Route::NativeN2 => plan_n2(src, items, rt, c),
        Route::Chihiro | Route::System246 => vec![],
    }
}

fn plan_es3b(
    src: &Path,
    items: &[Item],
    data_jp: &[Item],
    rt: &Path,
    c: &Components,
    op: &Path,
) -> Vec<Op> {
    let mut ops = vec![Op::Dir(rt.to_path_buf())];
    for it in items {
        if es3b_skip(&it.name) {
            continue;
        }
        let from = src.join(&it.name);
        let to = rt.join(&it.name);
        match (it.name.as_str(), it.dir) {
            // save folders OpenParrot redirects the game to: start empty
            ("TP" | "TeknoParrot", true) => ops.push(Op::Dir(to)),
            ("AMCUS" | "html", true) => ops.push(Op::TreeOnce { from, to }),
            ("data_jp", true) => {
                ops.push(Op::Dir(to.clone()));
                for child in data_jp {
                    let cf = from.join(&child.name);
                    let ct = to.join(&child.name);
                    if child.name == "network" {
                        // OpenParrot's WMMT6 module writes certificates here
                        ops.push(Op::TreeOnce { from: cf, to: ct });
                    } else {
                        ops.push(Op::Link { target: cf, at: ct });
                    }
                }
            }
            (_, true) => ops.push(Op::Link {
                target: from,
                at: to,
            }),
            (_, false) if it.size < SMALL => ops.push(Op::CopyOnce { from, to }),
            (_, false) => ops.push(Op::Link {
                target: from,
                at: to,
            }),
        }
    }
    for f in [
        "OpenParrot64.dll",
        "OpenParrotLoader64.exe",
        "iDmacDrv64.dll",
        "bngrw.dll",
    ] {
        ops.push(Op::Refresh {
            from: op.join(f),
            to: rt.join(f),
            exec: false,
        });
    }
    for f in ["d3d11.dll", "dxgi.dll"] {
        ops.push(Op::Refresh {
            from: c.dxvk.join(f),
            to: rt.join(f),
            exec: false,
        });
    }
    ops.push(Op::Refresh {
        from: c.io_board.join("wangan-io.exe"),
        to: rt.join("wangan-io.exe"),
        exec: false,
    });
    ops
}

fn plan_n2(src: &Path, items: &[Item], rt: &Path, c: &Components) -> Vec<Op> {
    let mut ops = vec![Op::Dir(rt.to_path_buf())];
    for it in items {
        let from = src.join(&it.name);
        let to = rt.join(&it.name);
        match it.name.as_str() {
            // the loader's files and the launcher-written config
            "libso" | "tmp" | "config.toml" | "keyconfig.toml" | "plugins" => {}
            // the share may not keep the execute bit: run a copy
            "main" => ops.push(Op::Refresh {
                from,
                to,
                exec: true,
            }),
            _ => ops.push(Op::Link {
                target: from,
                at: to,
            }),
        }
    }
    ops.push(Op::Dir(rt.join("libso")));
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

/// Carry out a plan. Idempotent; never touches a file the game wrote.
pub fn apply(ops: &[Op]) -> io::Result<()> {
    for op in ops {
        match op {
            Op::Dir(d) => fs::create_dir_all(d)?,
            Op::Link { target, at } => {
                if fs::symlink_metadata(at).is_ok() {
                    if fs::read_link(at).ok().as_deref() == Some(target.as_path()) {
                        continue;
                    }
                    if fs::symlink_metadata(at)?.file_type().is_symlink() {
                        fs::remove_file(at)?;
                    } else {
                        continue; // a real file/folder the game made: keep it
                    }
                }
                std::os::unix::fs::symlink(target, at)?;
            }
            Op::CopyOnce { from, to } => {
                if fs::symlink_metadata(to).is_err() && from.exists() {
                    copy_atomic(from, to)?;
                }
            }
            Op::TreeOnce { from, to } => {
                if fs::symlink_metadata(to).is_err() && from.is_dir() {
                    copy_tree(from, to)?;
                }
            }
            Op::Refresh { from, to, exec } => {
                let same = match (fs::read(from), fs::read(to)) {
                    (Ok(a), Ok(b)) => a == b,
                    _ => false,
                };
                if !same {
                    copy_atomic(from, to)?;
                }
                if *exec {
                    use std::os::unix::fs::PermissionsExt;
                    fs::set_permissions(to, fs::Permissions::from_mode(0o755))?;
                }
            }
        }
    }
    Ok(())
}

/// Whole-file swap (never write through an existing link).
fn copy_atomic(from: &Path, to: &Path) -> io::Result<()> {
    let tmp = to.with_extension("rea-tmp");
    let _ = fs::remove_file(&tmp);
    fs::copy(from, &tmp)?;
    if fs::symlink_metadata(to).is_ok() {
        fs::remove_file(to)?;
    }
    fs::rename(&tmp, to)
}

fn copy_tree(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for e in fs::read_dir(from)? {
        let e = e?;
        let t = to.join(e.file_name());
        if e.file_type()?.is_dir() || (e.file_type()?.is_symlink() && e.path().is_dir()) {
            copy_tree(&e.path(), &t)?;
        } else {
            fs::copy(e.path(), t)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{GameId, game};

    fn comps(base: &Path) -> Components {
        Components {
            openparrot: base.join("op"),
            openparrot_6rr: base.join("op6rr"),
            dxvk: base.join("dxvk"),
            io_board: base.join("io"),
            wal: base.join("wal"),
        }
    }

    fn item(name: &str, dir: bool, size: u64) -> Item {
        Item {
            name: name.into(),
            dir,
            size,
        }
    }

    #[test]
    fn es3b_plan_follows_the_runtime_rules() {
        let src = Path::new("/games/MT6");
        let rt = Path::new("/rt/mt6");
        let items = vec![
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
        ];
        let jp = vec![item("movie", true, 0), item("network", true, 0)];
        let ops = plan(
            game(GameId::Mt6),
            src,
            &items,
            &jp,
            rt,
            &comps(Path::new("/c")),
        );
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
        assert!(ops.contains(&Op::Link {
            target: src.join("data_jp/movie"),
            at: rt.join("data_jp/movie")
        }));
        assert!(ops.contains(&Op::TreeOnce {
            from: src.join("data_jp/network"),
            to: rt.join("data_jp/network")
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
        let src = Path::new("/g");
        let rt = Path::new("/rt");
        let items = vec![item("wmn6r.exe", false, 33_000_000)];
        let c = comps(Path::new("/c"));
        let rr = plan(game(GameId::Mt6rr), src, &items, &[], rt, &c);
        let r = plan(game(GameId::Mt6r), src, &items, &[], rt, &c);
        let dll = |ops: &[Op]| {
            ops.iter().find_map(|o| match o {
                Op::Refresh { from, to, .. } if to.ends_with("OpenParrot64.dll") => {
                    Some(from.clone())
                }
                _ => None,
            })
        };
        assert_eq!(dll(&rr), Some(PathBuf::from("/c/op6rr/OpenParrot64.dll")));
        assert_eq!(dll(&r), Some(PathBuf::from("/c/op/OpenParrot64.dll")));
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
        let items = list(&src).unwrap();
        let comps = Components {
            openparrot: c.join("op"),
            openparrot_6rr: c.join("op6rr"),
            dxvk: c.join("dxvk"),
            io_board: c.join("io"),
            wal: c.join("wal"),
        };
        let ops = plan(game(GameId::Mt5), &src, &items, &[], &rt, &comps);
        apply(&ops).unwrap();
        // the game writes its backup file; a later launch must keep it
        fs::write(rt.join("FBK.bin"), b"player save").unwrap();
        fs::write(c.join("op/OpenParrot64.dll"), b"v2").unwrap();
        apply(&ops).unwrap();
        assert_eq!(fs::read(rt.join("FBK.bin")).unwrap(), b"player save");
        assert_eq!(fs::read(rt.join("OpenParrot64.dll")).unwrap(), b"v2");
        assert!(rt.join("TP").is_dir());
        // and the source was never written
        assert_eq!(fs::read(src.join("FBK.bin")).unwrap(), b"dump");
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
