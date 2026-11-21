//! The Windows executable's version information, icon and manifest
//! (rea-launcher-build): a named, versioned program with the launcher's
//! icon instead of an anonymous one that antivirus heuristics distrust.
fn main() {
    rea_launcher_build::Resources::new(
        "Wangan Midnight",
        "One launcher for the Wangan Midnight arcade series",
    )
    .company("AkitaOnRails")
    .file_name("wangan-launcher.exe")
    .icon("../../packaging/appimage/wangan-launcher.png")
    .embed()
    .unwrap();
}
