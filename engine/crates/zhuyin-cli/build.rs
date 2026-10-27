//! With the `onnx` feature, records the system ONNX Runtime's directory (`ORT_LIB_PATH`, which also tells `ort-sys`
//! where to link it from) as the executable's run-time search path, as Swift's `Package.swift` does with
//! `ONNXRUNTIME_ROOT/lib`.

fn main() {
    println!("cargo:rerun-if-env-changed=ORT_LIB_PATH");
    if std::env::var_os("CARGO_FEATURE_ONNX").is_some()
        && let Ok(directory) = std::env::var("ORT_LIB_PATH")
        && std::env::var("CARGO_CFG_TARGET_OS").is_ok_and(|os| os != "windows")
    {
        println!("cargo:rustc-link-arg-bins=-Wl,-rpath,{directory}");
    }
}
