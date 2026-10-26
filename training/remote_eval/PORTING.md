# Platform boundaries and future input methods

ZhuyinCore is the shared, Foundation-only Swift engine. It has no model runtime or front-end dependency. A frontend translates native key events and preceding text into Composer calls, supplies model callbacks, then renders Presentation and applies commits. The CLI is the executable reference adapter. This change does not implement an input method or a C ABI.

## Runtime boundary and optimization

| Platform | Engine / front end | Model runtime and tuning |
|---|---|---|
| macOS (reference) | Swift; InputMethodKit, native AppKit UI | Core ML only, CPU by default; preserve CPU/ANE tuning, stateful context models, int8/palettized weights as future macOS-specific options. Existing `ZHUYIN_UNITS` CLI experiment remains available. No ONNX target links on macOS. |
| Linux (implemented CLI) | Same Swift engine; future fcitx5 or IBus adapter | ONNX Runtime C API, fp32 CPU, one intra/inter-op thread per CLI worker. Graph optimization enabled, cached context K/V, candidate batching within a reranker call. Test fp16 separately; do not replace the reference merely for a smaller file. The optional fp32 PyTorch batching server (`BACKEND=cuda`) is measured on the full evaluation matrix, bounded to 2 GiB of Torch allocations, and faster on this host; it is an evaluation optimization, while native Linux inference remains ONNX CPU. Future ONNX CUDA/OpenVINO execution providers must likewise be benchmarked including dispatch, startup and data loading. |
| Windows (planned) | Same Swift engine after platform validation; future TSF text service | ONNX Runtime CPU or DirectML, separately tuned. Use UTF-16 paths for C API session creation; ship the runtime DLLs beside the bridge. No Core ML/IMK/AppKit. Swift toolchain/Foundation availability and TSF COM apartment integration require a Windows build and tests. |

The small model-call interfaces are `NeuralReranker.Prediction` (flat int32 ids/targets, B, L -> flat logprob) and `SyllableScorer.Prediction` (context ids, syllable/tone/cost/boundary ids and candidate rows -> candidate logprobs). Common Swift code retains score mixing, exact-margin preference, 24/255-character context limits, padding, unknown-character penalty, candidate truncation and ordering. Model runtimes implement tensor calls only. v3 projected K/V is cached and passed directly; there is no second projection. v3 models normalize over the supplied candidate subset, not the full vocabulary.

The shipped Core ML packages use fp16 arithmetic; ONNX fp32 weights still preserve the fp16 memory interface between the v3 encoders. `*.fp16.onnx` experiments keep external input/output types identical. Models must come from a matching checkpoint pair. Linux errors during model loading are fatal; shared scorer inference failures retain the reference API's nil/fallback behavior. Run parity and smoke checks before publishing artifacts.

## Proposed C ABI (new target, not yet built)

Add a separate `ZhuyinC` target depending on ZhuyinCore and the platform's model module. Export only fixed-width C types, opaque handles, explicit ownership and versioned functions through `@_cdecl`; never expose Swift/Foundation objects or depend on Swift struct layout.

* `zt_engine_create(data_path_utf8, options_json, error_out)` / `zt_engine_destroy`: load an immutable engine and runtime bundle; explicitly opt into a per-user learning directory, defaulting to no writes for evaluation.
* `zt_composer_create(engine_handle)` / `zt_composer_destroy`: a composition session and its context. Define whether the engine is shared or owned; initially use one engine per input session to avoid sharing mutable learning/domain state.
* `zt_set_context(handle, utf8, byte_length)`, `zt_key(handle, key_code, modifiers, monotonic_seconds)`, `zt_reset(handle)`, `zt_break_context(handle)`: key and focus lifecycle, including timed-key decoding. Define a stable key enum rather than platform key codes. Frontends retain native key mappings.
* `zt_presentation_json(handle)` / `zt_string_free`: return UTF-8 presentation/commit/result records with explicit byte lengths, handled/pass-through status, candidate page/selection, clauses, notices and UTF-16 cursor offsets. A first JSON ABI is adequate at input-method event rates; add a binary view only after measurement.
* Candidate selection, page navigation and editing use the existing Composer key API. Learning exclusions/import/export need separately versioned calls once a frontend actually needs them.
* Every export catches Swift errors and returns a status plus owned UTF-8 error; no exception, trap or Swift allocator crosses the ABI. Define library ABI version and data/model format versions, plus cancellation/shutdown ordering.

## Threading

Composer/Engine contain mutable composition, learning, cache and domain state. Serialize all calls for a handle on one frontend event queue; never call one Composer concurrently. Load large data/models off the UI/TSF thread, then publish a ready handle. Model sessions have their own locks; those locks do not make Composer thread-safe. The Linux evaluation baseline uses independent processes and resets per line. A future shared runtime/server may reuse immutable weights, but batching must preserve context, bidirectional syllable lengths, padding masks and candidate normalization exactly.

TSF calls must marshal to the owning COM apartment; fcitx5/IBus integration must preserve synchronous handled/pass-through semantics even if loading occurs in the background. Document time units and UTF-8 versus UTF-16 indices explicitly.

## Data files, licensing and packaging

Bundle lexicon.tsv; lm and optional lm_chat unigram/bigram/backoff files; reranker vocab/config plus platform model; v3 chars/syllables/config plus matching encoder pair. Preserve model/source NOTICE, provenance, licenses and Wikipedia attribution manifests. Runtime inference needs exported models and JSON, not training checkpoints or raw corpora. The remote runner sends checkpoints and exports on Linux; Core ML bundles remain on the Mac.

* macOS: keep current .app/.pkg bundle, signing and InputMethodKit lifecycle. No packaging script changes in this task.
* Linux: package Swift runtime libraries, the C bridge, ONNX Runtime .so and notices with a versioned data directory; use relocatable loader paths in a distributable package (the evaluation build uses `ONNXRUNTIME_ROOT` and an absolute rpath). Add fcitx5/IBus registration and desktop integration in a future frontend package. Engine/model artifacts are separately checksum/versioned.
* Windows: signed TSF DLL and registration/unregistration, compatible Swift runtime and ONNX Runtime/DirectML DLLs, UTF-16 paths, per-user AppData learning storage; verify upgrade, focus and process shutdown behavior on real applications. Unix socket `ModelServer.swift` is an experimental evaluator path and must be excluded or replaced for Windows.

Native frontends and C ABI implementation are deliberately deferred. Neither a Windows build nor Linux desktop input integration is claimed here.
