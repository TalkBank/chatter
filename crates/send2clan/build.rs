//! Build script for compiling and linking the `send2clan` C library.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>

/// Build scripts execute on the host; linking follows Cargo's target instead.
enum NativeLinkage {
    AppleFrameworks,
    WindowsLibraries,
    NoAdditionalLibraries,
}

impl NativeLinkage {
    fn from_cargo() -> Result<Self, std::env::VarError> {
        Ok(match std::env::var("CARGO_CFG_TARGET_OS")?.as_str() {
            "macos" => Self::AppleFrameworks,
            "windows" => Self::WindowsLibraries,
            _ => Self::NoAdditionalLibraries,
        })
    }
}

/// Entry point for this binary target.
fn main() -> Result<(), std::env::VarError> {
    let linkage = NativeLinkage::from_cargo()?;
    let mut build = cc::Build::new();

    // Compile the C library
    build
        .file("csrc/send2clan.c")
        .include("csrc")
        .warnings(true)
        .extra_warnings(true);

    if let NativeLinkage::WindowsLibraries = linkage {
        build.define("_WIN32", None);
        // We always compile send2clan as a static library (cc::Build::compile
        // produces a static archive). Without SEND2CLAN_STATIC_BUILD, the
        // SEND2CLAN_API macro in send2clan.h resolves to __declspec(dllimport),
        // which conflicts with the function *definitions* in send2clan.c and
        // causes MSVC error C2491.
        build.define("SEND2CLAN_STATIC_BUILD", None);
    }

    build.compile("send2clan");

    // Platform-specific linking
    match linkage {
        NativeLinkage::AppleFrameworks => {
            println!("cargo:rustc-link-lib=framework=Carbon");
            println!("cargo:rustc-link-lib=framework=ApplicationServices");
            println!("cargo:rustc-link-lib=framework=CoreServices");
        }
        NativeLinkage::WindowsLibraries => {
            println!("cargo:rustc-link-lib=shell32");
            println!("cargo:rustc-link-lib=user32");
            println!("cargo:rustc-link-lib=advapi32");
        }
        NativeLinkage::NoAdditionalLibraries => {}
    }

    // Re-run build script if C source changes
    println!("cargo:rerun-if-changed=csrc/send2clan.c");
    println!("cargo:rerun-if-changed=csrc/send2clan.h");
    Ok(())
}
