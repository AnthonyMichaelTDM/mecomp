//! On macOS, the statically linked ONNX Runtime (`CoreML` EP) references
//! `___isPlatformVersionAtLeast`, which lives in clang's compiler-rt. rustc links with
//! `-nodefaultlibs`, so we have to link the runtime explicitly.
//!
//! Theoretically, none of this should be necessary (see <https://github.com/pykeio/ort/pull/625>)
//! However, before adding this publishing v0.7.3 kept failing on macOS with linking errors similar to
//! those reported in <https://github.com/pykeio/ort/issues/360>
use std::{path::Path, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }

    let Ok(output) = Command::new("cc")
        .arg("--print-file-name=libclang_rt.osx.a")
        .output()
    else {
        return;
    };
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let path = Path::new(&path);

    // `--print-file-name` echoes the bare name back if the file isn't found
    if path.is_absolute()
        && path.exists()
        && let Some(dir) = path.parent()
    {
        println!("cargo:rustc-link-search=native={}", dir.display());
        println!("cargo:rustc-link-lib=static=clang_rt.osx");
    }
}
