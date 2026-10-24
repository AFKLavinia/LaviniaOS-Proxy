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
    build.compile("lavinia_tun");

    println!("cargo:rerun-if-changed=../src/c");
    println!("cargo:rerun-if-changed=build.rs");

    tauri_build::build();
}
