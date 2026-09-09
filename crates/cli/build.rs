use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/device_auth_macos.m");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("out dir"));
    let source = manifest_dir.join("src/device_auth_macos.m");
    let object = out_dir.join("device_auth_macos.o");
    let library = out_dir.join("libfresnica_device_auth_macos.a");
    let arch = match env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
        Ok("aarch64") => "arm64",
        Ok("x86_64") => "x86_64",
        Ok(other) => panic!("unsupported macOS target architecture: {other}"),
        Err(error) => panic!("unable to determine macOS target architecture: {error}"),
    };

    run(
        Command::new("xcrun")
            .args([
                "--sdk",
                "macosx",
                "clang",
                "-fobjc-arc",
                "-fblocks",
                "-arch",
                arch,
                "-c",
            ])
            .arg(&source)
            .arg("-o")
            .arg(&object),
        "compile macOS LocalAuthentication shim",
    );
    run(
        Command::new("xcrun")
            .args(["--sdk", "macosx", "ar", "rcs"])
            .arg(&library)
            .arg(&object),
        "archive macOS LocalAuthentication shim",
    );

    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=fresnica_device_auth_macos");
    println!("cargo:rustc-link-lib=framework=LocalAuthentication");
    println!("cargo:rustc-link-lib=framework=Foundation");
}

fn run(command: &mut Command, action: &str) {
    let status = command
        .status()
        .unwrap_or_else(|error| panic!("unable to {action}: {error}"));
    assert!(status.success(), "failed to {action}: {status}");
}
