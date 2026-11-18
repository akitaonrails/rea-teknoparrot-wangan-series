//! The command line: help, bad input, and options landing in the same
//! settings file the window writes.

use std::process::Command;

/// The launcher with no display and a lock of its own (these runs have no
/// window: a refusal is never a message window).
fn launcher(data: &std::path::Path) -> Command {
    let run_dir = data.join("run");
    std::fs::create_dir_all(&run_dir).unwrap();
    let mut c = Command::new(env!("CARGO_BIN_EXE_wangan-launcher"));
    c.env("WANGAN_DATA", data)
        .env("WANGAN_COMPONENTS", data.join("none"))
        .env("XDG_RUNTIME_DIR", run_dir)
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY");
    c
}

#[test]
fn help_lists_every_option() {
    let t = tempfile::tempdir().unwrap();
    let out = launcher(t.path()).arg("--help").output().unwrap();
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    for flag in [
        "--library",
        "--list",
        "--play",
        "--set",
        "--check",
        "--add-files",
        "--look",
        "--output",
        "--upscaler",
        "--sharpness",
        "--fullscreen",
        "--render",
        "--deadzone",
    ] {
        assert!(text.contains(flag), "{flag} missing from --help");
    }
}

#[test]
fn unknown_option_and_bad_value_exit_2() {
    let t = tempfile::tempdir().unwrap();
    assert_eq!(
        launcher(t.path())
            .arg("--nope")
            .output()
            .unwrap()
            .status
            .code(),
        Some(2)
    );
    let bad = launcher(t.path())
        .args(["--game", "mt5", "--deadzone", "0.9"])
        .output()
        .unwrap();
    assert_eq!(bad.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&bad.stderr).contains("bad value for --deadzone"));
    assert_eq!(
        launcher(t.path())
            .args(["--game", "nope", "--output", "1920x1080"])
            .output()
            .unwrap()
            .status
            .code(),
        Some(2)
    );
}

#[test]
fn options_are_saved_like_the_window_saves_them() {
    let t = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(t.path().join("settings")).unwrap();
    std::fs::write(
        t.path().join("settings/mt3dxp.cfg"),
        "# mine\nsharpness = 7\n",
    )
    .unwrap();
    // gamescope's options exist on Linux only (the window shows them off on Windows)
    let gamescope: &[&str] = if cfg!(windows) {
        &[]
    } else {
        &[
            "--output",
            "2560x1440",
            "--upscaler",
            "nis",
            "--fullscreen",
            "off",
        ]
    };
    let st = launcher(t.path())
        .args(["--game", "mt3dxp"])
        .args(gamescope)
        .args(["--render", "3840x2160", "--deadzone", "0.1"])
        .status()
        .unwrap();
    assert!(st.success());
    let text = std::fs::read_to_string(t.path().join("settings/mt3dxp.cfg")).unwrap();
    let (s, problems) = wangan_core::settings::Settings::parse(&text);
    assert!(problems.is_empty(), "{problems:?}");
    assert!(text.starts_with("# mine\nsharpness = 7\n"), "{text}");
    if cfg!(windows) {
        let off = launcher(t.path())
            .args(["--game", "mt5", "--output", "2560x1440"])
            .output()
            .unwrap();
        assert_eq!(off.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&off.stderr).contains("mt5 doesn't take --output"));
    } else {
        assert_eq!(s.output.to_string(), "2560x1440");
        assert_eq!(s.upscaler.key(), "nis");
        assert!(!s.fullscreen);
    }
    assert_eq!(s.render.unwrap().to_string(), "3840x2160");
    assert!((s.deadzone - 0.1).abs() < 1e-6);
    assert_eq!(s.sharpness, 7);
}
