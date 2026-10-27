// Which engine the app is built on.  The Swift engine (ZhuyinCore, ZhuyinML) by default; with ZHUYIN_ENGINE=rust
// (Package.swift) the Rust engine through ZhuyinRust, under the same names, so the rest of the app is unchanged.
#if ZHUYIN_RUST_ENGINE
import ZhuyinRust

typealias Engine = RustEngine
typealias Composer = RustComposer
typealias NeuralReranker = RustReranker
typealias SyllableScorer = RustSyllableScorer
#endif
