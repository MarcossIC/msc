use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=msc-shim/src/main.rs");
    println!("cargo:rerun-if-changed=msc-shim/Cargo.toml");

    // Get the output directory
    let out_dir = env::var("OUT_DIR").unwrap();
    let profile = env::var("PROFILE").unwrap();

    // Determine the target directory based on profile
    let target_dir = if profile == "release" {
        "release"
    } else {
        "debug"
    };

    println!("cargo:warning=Building msc-shim for profile: {}", profile);

    // Build the shim project
    let mut build_cmd = Command::new("cargo");
    build_cmd.current_dir("msc-shim").arg("build");

    // Only use --release if we're building in release mode
    if profile == "release" {
        build_cmd.arg("--release");
    }

    let status = build_cmd
        .status()
        .expect("Failed to execute cargo build for msc-shim");

    if !status.success() {
        panic!("Failed to build msc-shim");
    }

    println!(
        "cargo:warning=msc-shim built successfully in {} mode",
        profile
    );

    // Get the path to the compiled shim
    let shim_source = PathBuf::from("msc-shim").join("target").join(target_dir);

    #[cfg(target_os = "windows")]
    let shim_exe = shim_source.join("msc-shim.exe");

    #[cfg(not(target_os = "windows"))]
    let shim_exe = shim_source.join("msc-shim");

    // Verify the shim was built
    if !shim_exe.exists() {
        panic!("Shim executable not found at {:?}", shim_exe);
    }

    println!("cargo:warning=Shim found at: {:?}", shim_exe);

    // Copy the shim to the OUT_DIR so it can be included
    let out_path = PathBuf::from(&out_dir);

    #[cfg(target_os = "windows")]
    let shim_dest = out_path.join("msc-shim.exe");

    #[cfg(not(target_os = "windows"))]
    let shim_dest = out_path.join("msc-shim");

    std::fs::copy(&shim_exe, &shim_dest).expect("Failed to copy shim to output directory");

    println!("cargo:warning=Shim copied to: {:?}", shim_dest);

    // Tell cargo where to find the shim
    println!("cargo:rustc-env=MSC_SHIM_PATH={}", shim_dest.display());
}
