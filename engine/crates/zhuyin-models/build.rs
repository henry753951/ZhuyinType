//! With the `onnx` feature, records the system ONNX Runtime's directory (`ORT_LIB_PATH`) as the test executables'
//! run-time search path, so `cargo test --features onnx` runs without `LD_LIBRARY_PATH`.

fn main() {
    println!("cargo:rerun-if-env-changed=ORT_LIB_PATH");
    if std::env::var_os("CARGO_FEATURE_ONNX").is_some()
        && let Ok(directory) = std::env::var("ORT_LIB_PATH")
        && std::env::var("CARGO_CFG_TARGET_OS").is_ok_and(|os| os != "windows")
    {
        println!("cargo:rustc-link-arg-tests=-Wl,-rpath,{directory}");
    }
}
