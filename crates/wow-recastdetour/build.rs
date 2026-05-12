use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be set by Cargo"),
    );
    let detour_dir = manifest_dir.join("vendor/Detour");
    let include_dir = detour_dir.join("Include");
    let source_dir = detour_dir.join("Source");

    for path in [
        "DetourAlloc.cpp",
        "DetourAssert.cpp",
        "DetourCommon.cpp",
        "DetourNavMesh.cpp",
        "DetourNavMeshBuilder.cpp",
        "DetourNavMeshQuery.cpp",
        "DetourNode.cpp",
    ] {
        println!("cargo:rerun-if-changed={}", source_dir.join(path).display());
    }

    for path in [
        "DetourAlloc.h",
        "DetourAssert.h",
        "DetourCommon.h",
        "DetourMath.h",
        "DetourNavMesh.h",
        "DetourNavMeshBuilder.h",
        "DetourNavMeshQuery.h",
        "DetourNode.h",
        "DetourStatus.h",
    ] {
        println!(
            "cargo:rerun-if-changed={}",
            include_dir.join(path).display()
        );
    }

    cc::Build::new()
        .cpp(true)
        .std("c++17")
        .include(&include_dir)
        .flag_if_supported("-Wno-class-memaccess")
        .files([
            source_dir.join("DetourAlloc.cpp"),
            source_dir.join("DetourAssert.cpp"),
            source_dir.join("DetourCommon.cpp"),
            source_dir.join("DetourNavMesh.cpp"),
            source_dir.join("DetourNavMeshBuilder.cpp"),
            source_dir.join("DetourNavMeshQuery.cpp"),
            source_dir.join("DetourNode.cpp"),
        ])
        .compile("wow_detour");
}
