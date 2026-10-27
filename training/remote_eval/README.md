# Portable CLI and remote evaluations

The Mac stays on Core ML. Linux uses the same Swift engine and scoring code with ONNX Runtime's C API. See [REPORT.md](REPORT.md) for measured parity/timing and [PORTING.md](PORTING.md) for platform tuning and the future C ABI/frontends.

## Run

From the Mac repository root (SSH key and host default to the lab machine):

```sh
JOBS=60 training/remote_eval/run.sh conversion_eval -n 400 \
  --reranker data/build/reranker --rerank-count 16 \
  --cli-arg=--v3 --cli-arg=data/build/v3 --cli-arg=--v3-weight --cli-arg=1 \
  --test data/build/corpus/dev_colloquial.txt data/build/corpus/dev_wikipedia.txt data/build/lm_chat/dev_chat_tt.txt

JOBS=60 training/remote_eval/run.sh sweep --matrix training/remote_eval/validation.json -n 400
# Faster optional lab-host evaluation (measured; requires free GPU memory):
BACKEND=cuda JOBS=60 training/remote_eval/run.sh sweep --matrix training/remote_eval/validation.json -n 400

JOBS=60 training/remote_eval/run.sh error_analysis -n 400 --reranker data/build/reranker \
  --test data/build/corpus/dev_colloquial.txt
JOBS=60 training/remote_eval/run.sh typing_errors -n 400 --json data/build/remote_eval/typing-errors.json
JOBS=60 training/remote_eval/run.sh typo_eval -n 400 --reranker data/build/reranker \
  --json data/build/remote_eval/typos.json
```

`run.sh` syncs only portable sources, evaluator sources, lexicon/LM/evaluation files, JSON and model checkpoints to the isolated `~/zhuyintype/work/onnx-eval`. It does not sync Core ML artifacts or write into the training workspace. Until the reranker checkpoint has been retrieved to local `data/build/reranker/charlm.pt`, it copies the verified remote `reranker_v2b/charlm.pt` into the isolated directory. It builds and exports there (exports are cached by input-content hashes). Logs, sweep predictions and JSON outputs under remote `data/build/remote_eval/` are retrieved into local `data/build/remote_eval/remote/`, including on failure. A nonzero build/export/evaluation exits immediately.

`HOST`, `KEY`, `REMOTE_DIR` (restricted to the isolated prefix), `JOBS`, and `BACKEND=cpu|cuda` are overridable. `cpu` is the default ONNX C API path. `cuda` starts the measured fp32 PyTorch batching server with a 2 GiB allocation limit, passes its private socket to every worker, and cleans it up on exit. CUDA is useful for large lab-host sweeps; CPU needs no GPU capacity. Avoid overlapping runner invocations against the same `REMOTE_DIR`. Other CLI workers can coexist with the training job. Each ONNX session uses one CPU thread. `evaluate.py` partitions independent CLI batch lines for the three tools that lack `--jobs`, preserving order, errors and their original output. conversion_eval uses its existing `--jobs`. Stateful `--keep-memory` batches are rejected by the parallel adapter.

`TREE_RERANKER=1` also exports `RerankerTree.onnx` with unchanged v2b weights. The native CPU scorer uses it automatically when present, with batched scoring as fallback. Keep a separate directory containing only `Reranker.onnx`, `vocab.json`, and `config.json` for a baseline comparison. Tree exports are opt-in so other concurrent sweeps retain their existing export behavior. See [REPORT_tree.md](../reranker/REPORT_tree.md) for the app-configuration matrix, score parity, and Mac timing commands. The optional CUDA socket server continues to use batched scoring.
`TREE_RERANKER=0` does not remove or disable an already-present tree model; selection depends on model presence.

## Named sweeps

A matrix is JSON with `configurations: [{"name": "…", "args": ["--flag", "value", …]}]` and `sets: [{"name": "…", "path": "…", "args": ["--line-context"]}]`. Set args are conversion_eval options; configuration args are CLI options. For continued compositions use `"args": ["--split", "continue"]`. See `validation.json`. Names use letters, digits, `_` and `-`. The runner executes the Cartesian product and prints one table. `--jobs` controls worker count within each row. Models and engine flags are otherwise unrestricted. Outputs include each ordered CLI input and prediction, per-set accuracy and whole evaluation wall time (including Python preparation and CLI startup).

## Remote setup (already performed on the lab host)

Use the existing Swift 6.4.1 toolchain at `~/opt/swift/usr/bin`; the official Ubuntu 26.04 build works on this host. Install the official ONNX Runtime **1.24.4** Linux x64 SDK into `~/opt/onnxruntime-linux-x64-1.24.4` and symlink `~/opt/onnxruntime` to it. `ONNXRUNTIME_ROOT` overrides the SDK path at build time; its `include/` and `lib/` must exist. Package.swift links the bridge only on Linux and embeds the SDK lib path for evaluation.

```sh
cd ~/zhuyintype/work/onnx-eval
~/.local/bin/uv venv .venv --python ~/zhuyintype/.venv-gen/bin/python
printf '%s\n' "$HOME/zhuyintype/.venv-gen/lib/python3.12/site-packages" > .venv/lib/python3.12/site-packages/training-torch.pth
~/.local/bin/uv pip install --python .venv/bin/python -r training/remote_eval/requirements.txt
PATH="$HOME/opt/swift/usr/bin:$PATH" swift build -c release
```

All install commands, sources and resolved versions are recorded in REPORT.md. The separate environment reuses Torch without changing `.venv-gen` or the other training jobs. No Python/global Mac toolchain configuration is needed.

## Export and checks

```sh
.venv/bin/python training/remote_eval/export_onnx.py \
  --reranker-checkpoint data/build/reranker/charlm.pt --fp16
# Writes fp32 baseline and optional *.fp16.onnx experiments next to model JSON.
.venv/bin/python training/remote_eval/parity.py --output data/build/remote_eval/parity.json
PATH="$HOME/opt/swift/usr/bin:$PATH" swift test -c release --filter BackendTests
python3 training/remote_eval/test_tools.py
# Linux socket experiment regression (uses the built CLI and existing lexicon):
python3 training/remote_eval/test_socket.py
```

Parity expects the existing `data/build/v3/test_vectors.json` and `data/build/remote_eval/reranker-coreml.json`. Create the latter on the Mac using `data/build/v3/.venv/bin/python training/remote_eval/parity.py --coreml --output data/build/remote_eval/reranker-coreml.json`, then transfer it. No training source files are edited by export/parity tools. PyTorch imports are read-only; set `PYTHONDONTWRITEBYTECODE=1` if the training directories must have no cache writes.

For a fresh parity study, freeze sources/data first, build the Mac CLI with Xcode, run the same matrix on both platforms, compare ordered inputs before counting output differences, and save manifests. `compare.py` prints the final combined table and exits nonzero if any row exceeds 1% differing predictions. Existing scratch snapshots are evidence, not inputs to future runs.

The optional `gpu_benchmark.sh` uses the preserved fp32 PyTorch socket server with a 2 GiB CUDA allocation cap. It runs the same complete matrix and cleans up only its own server. The full measured sweep was 393.13 s with this server versus 610.12 s with ONNX CPU; all 6,000 GPU/CPU predictions matched. Enable it explicitly with `BACKEND=cuda`; the native portable runtime remains ONNX CPU by default.
