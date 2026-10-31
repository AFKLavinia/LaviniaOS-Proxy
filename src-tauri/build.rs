use std::env;

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let mut build = cc::Build::new();
    build
        .include("../src/c/include")
        .file("../src/c/ring_buffer.c")
        .file("../src/c/tun_core.c")
        .warnings(true);

    let platform_src = match target_os.as_str() {
        "linux" => "../src/c/platform/linux/tun_linux.c",
        "windows" => "../src/c/platform/windows/tun_windows.c",
        other => panic!("unsupported target_os: {other}"),
    };
    build.file(platform_src);

    if build.get_compiler().is_like_msvc() {
        build.flag("/std:c11");
        build.flag("/experimental:c11atomics");
        build.define("_CRT_SECURE_NO_WARNINGS", None);
    } else {
        build.std("c11");
    }

    if target_os == "linux" {
        build.flag("-pthread");
        println!("cargo:rustc-link-lib=pthread");
    }

    build.compile("lavinia_tun");

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../src/c/include/lavinia_tun.h");
    println!("cargo:rerun-if-changed=../src/c/ring_buffer.h");
    println!("cargo:rerun-if-changed=../src/c/ring_buffer.c");
    println!("cargo:rerun-if-changed=../src/c/tun_core.c");
    println!("cargo:rerun-if-changed=../src/c/platform/platform.h");
    println!("cargo:rerun-if-changed=../src/c/platform/linux/tun_linux.c");
    println!("cargo:rerun-if-changed=../src/c/platform/windows/tun_windows.c");

    tauri_build::build();
}
