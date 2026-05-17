use std::env;
use std::process::Command;

fn rustflags_request_native() -> bool {
    fn iter_flags(var: &str) -> Vec<String> {
        env::var(var)
            .ok()
            .map(|v| {
                if var == "CARGO_ENCODED_RUSTFLAGS" {
                    v.split('\x1f')
                        .filter(|s| !s.is_empty())
                        .map(|s| s.to_string())
                        .collect()
                } else {
                    v.split_whitespace().map(|s| s.to_string()).collect()
                }
            })
            .unwrap_or_default()
    }

    let mut flags = iter_flags("CARGO_ENCODED_RUSTFLAGS");
    flags.extend(iter_flags("RUSTFLAGS"));

    for (i, flag) in flags.iter().enumerate() {
        if flag == "-Ctarget-cpu=native" {
            return true;
        }

        if flag == "-C"
            && let Some(next) = flags.get(i + 1)
            && next == "target-cpu=native"
        {
            return true;
        }
    }

    false
}

fn exe_exists(name: &str) -> bool {
    #[cfg(unix)]
    {
        Command::new("sh")
            .arg("-lc")
            .arg(format!("command -v {name} >/dev/null 2>&1"))
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    #[cfg(windows)]
    {
        Command::new("where")
            .arg(name)
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    #[cfg(not(any(unix, windows)))]
    {
        false
    }
}

fn main() {
    println!("cargo:rerun-if-changed=zpaq/libzpaq.cpp");
    println!("cargo:rerun-if-changed=zpaq/libzpaq.h");
    println!("cargo:rerun-if-changed=zpaq_rs_ffi.cpp");

    let mut build = cc::Build::new();

    let target = env::var("TARGET").unwrap_or_default();
    let host = env::var("HOST").unwrap_or_default();
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

    // Toolchain selection:
    // - If `ZPAQ_RS_CXX` is set, use it.
    // - Else, let the `cc` crate honor CXX/CXX_$target/its platform default.
    // - For native Linux builds only, prefer g++ over clang++ when no CXX is set.
    //   Release builds in this workspace link with rust-lld; clang++ can emit
    //   LLVM metadata newer than rust-lld understands.
    if let Ok(cxx) = env::var("ZPAQ_RS_CXX") {
        if !cxx.trim().is_empty() {
            build.compiler(cxx);
        }
    } else if env::var_os("CXX").is_none()
        && target_os == "linux"
        && target == host
        && exe_exists("g++")
    {
        build.compiler("g++");
    }

    build
        .cpp(true)
        .include("zpaq")
        .file("zpaq/libzpaq.cpp")
        .file("zpaq/zpaq.cpp")
        .file("zpaq_rs_ffi.cpp")
        // zpaq.cpp contains a `main()` (or `wmain()` on Windows). Rename it so it can be linked into this library.
        .define("main", "zpaq_cli_main")
        .define("wmain", "zpaq_cli_main")
        .flag_if_supported("-std=c++17")
        .flag_if_supported("-fvisibility=hidden")
        .flag_if_supported("-fPIC")
        .flag_if_supported("-pthread")
        .flag_if_supported("-Wno-unused-parameter")
        .flag_if_supported("-Wno-null-pointer-subtraction")
        .flag_if_supported("-Wno-unused-const-variable")
        .define("NDEBUG", None);

    // Only define unix on UNIX systems (not on Windows)
    #[cfg(unix)]
    build.define("unix", None);

    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();

    // libzpaq JIT is x86_64-specific. Force NOJIT on non-x86_64 targets,
    // while still allowing explicit `--features nojit` on x86_64.
    if env::var_os("CARGO_FEATURE_NOJIT").is_some() || target_arch != "x86_64" {
        build.define("NOJIT", None);
    }

    // Always optimize zpaq
    build.flag_if_supported("-O3");

    // Keep C++ codegen aligned with Rust when native tuning is explicitly requested.
    if rustflags_request_native() {
        match target_arch.as_str() {
            "x86" | "x86_64" => {
                build.flag_if_supported("-march=native");
            }
            "arm" | "aarch64" => {
                build.flag_if_supported("-mcpu=native");
            }
            _ => {}
        }
    }

    build.compile("zpaq_rs_ffi");

    // On Windows, zpaq needs advapi32 for CryptoAPI (CryptAcquireContext, etc.)
    #[cfg(windows)]
    println!("cargo:rustc-link-lib=advapi32");
}
