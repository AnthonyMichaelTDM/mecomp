fn main() {
    println!("cargo:rustc-check-cfg=cfg(tarpaulin_include)");
}
