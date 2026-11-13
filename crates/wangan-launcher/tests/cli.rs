//! The command line: help, bad input, and options landing in the same
//! settings file the window writes.

use std::process::Command;

fn launcher(data: &std::path::Path) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_wangan-launcher"));
    c.env("WANGAN_DATA", data)
        .env("WANGAN_COMPONENTS", data.join("none"))
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
        "--game",
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
        .args(["--game", "mt5", "--upscaler", "dlss"])
        .output()
        .unwrap();
    assert_eq!(bad.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&bad.stderr).contains("bad value for --upscaler"));
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
    let st = launcher(t.path())
        .args([
            "--game",
            "mt3dxp",
            "--output",
            "2560x1440",
            "--upscaler",
            "nis",
            "--fullscreen",
            "off",
            "--render",
            "3840x2160",
            "--deadzone",
            "0.1",
        ])
        .status()
        .unwrap();
    assert!(st.success());
    let text = std::fs::read_to_string(t.path().join("settings/mt3dxp.cfg")).unwrap();
    let (s, problems) = wangan_core::settings::Settings::parse(&text);
    assert!(problems.is_empty(), "{problems:?}");
    assert!(text.starts_with("# mine\nsharpness = 7\n"), "{text}");
    assert_eq!(s.output.to_string(), "2560x1440");
    assert_eq!(s.upscaler.key(), "nis");
    assert!(!s.fullscreen);
    assert_eq!(s.render.unwrap().to_string(), "3840x2160");
    assert!((s.deadzone - 0.1).abs() < 1e-6);
    assert_eq!(s.sharpness, 7);
}
