# Portable engine and remote evaluation report

2026-10-27 (Asia/Taipei). CPU/GPU evaluation paths, model parity, both complete 15-row platform comparisons and both remote runner modes are validated. Known latency failures are retained explicitly.

## Starting state and boundaries

The working tree already contained concurrent engine/app changes, conditional Package.swift/Core ML imports, prediction injection initializers, a Python GPU socket backend and an active Mac matrix. These were preserved. No commits, resets, stashes, app sources, docs, build-app script or other training directories are edited by this task. The isolated remote destination is `~/zhuyintype/work/onnx-eval`; the homophone workspace and its GPU process are untouched.

Fresh baseline: `DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer swift build -c release --scratch-path .build-codex2` passed, then checks reported **3072/3083, 11 latency-only failures**, not the anticipated six. Exact log: `data/build/remote_eval/baseline/checks.log`. The core sources are unchanged by this task. Latency counts vary under concurrent evaluation load; behavioral regressions must be distinguished from those timing thresholds.

Remote: Ubuntu 26.04.1 x86_64; Swift 6.4.1; PyTorch 2.13.0+cu130; 64 CPUs / approximately 125 GB RAM. Initial GPU use 43549 MiB of 46587 MiB, by the existing VLLM process. CPU inference is the baseline and uses no GPU memory.

## Installs

No Mac environment, global runtime defaults or training environment was modified.

* Swift **6.4.1**, already installed by the preceding attempt into `~/opt/swift-6.4.1-RELEASE-ubuntu26.04` with `~/opt/swift` symlink. Official source: `https://download.swift.org/swift-6.4.1-release/ubuntu2604/swift-6.4.1-RELEASE/swift-6.4.1-RELEASE-ubuntu26.04.tar.gz`. Historical install output is `training/remote_eval/results/swift-install.log`; this run verified `~/opt/swift/usr/bin/swift --version`. No reinstallation.
* ONNX Runtime C SDK **1.24.4**, official Microsoft release: `curl -fL https://github.com/microsoft/onnxruntime/releases/download/v1.24.4/onnxruntime-linux-x64-1.24.4.tgz -o ~/opt/onnxruntime-linux-x64-1.24.4.tgz`; `tar -xzf ~/opt/onnxruntime-linux-x64-1.24.4.tgz -C ~/opt`; `ln -sfn onnxruntime-linux-x64-1.24.4 ~/opt/onnxruntime`.
* An isolated Python **3.12.15** environment, using the existing uv **0.13.5**: `~/.local/bin/uv venv ~/zhuyintype/work/onnx-eval/.venv --python ~/zhuyintype/.venv-gen/bin/python`. A `training-torch.pth` points to the existing `.venv-gen/lib/python3.12/site-packages` to reuse Torch without modifying it.
* PyPI packages: `~/.local/bin/uv pip install --python ~/zhuyintype/work/onnx-eval/.venv/bin/python onnx==1.20.1 onnxruntime==1.24.4 onnxconverter-common==1.16.0`. Resolved dependencies: flatbuffers 25.12.19, ml-dtypes 0.6.0, mpmath 1.3.0, numpy 2.5.4, packaging 26.3, protobuf 7.36.2, sympy 1.14.0, typing-extensions 4.16.0. Sources: PyPI. No sudo/system install was needed.

## Implementation and validation

Measured results and limitations follow; raw artifacts are under data/build/remote_eval/.

### First verified results

* Mac all-product release build passed. Linux `swift build -c release` passed without any ZhuyinCore compatibility edits. Both platforms passed the two injected-backend XCTest checks (reranker encoding/context/padding/OOV/batching/exactMargin; syllable context/newline/candidate truncation/mapping/output validation). Three Python adapter regression checks passed.
* `manifest-mac.json` and `manifest-linux.json` match all **43** frozen source/data files. Mac snapshot: `data/build/remote_eval/frozen`; remote snapshot: `~/zhuyintype/work/onnx-eval`. Manifest excludes AppleDouble archive metadata and unrelated v3 smoke models. Full test-vector inputs, vocabulary JSON, checkpoints, evaluation sets and LM files are included.
* fp32 ONNX vs PyTorch: v3 maximum valid logprob absolute error **0.0000181198** over all 20 existing golden vectors. Maximum vs CPU Core ML **0.0729847**, within the existing PyTorch-vs-fp16 tolerance of 0.1; individual near-tie top-1 differences exist in the vectors. The reranker matches the shipped Mac model on real sentence/long context/one-character/OOV inputs: maximum ONNX-vs-Core-ML logprob error **0.0675192** (verifies v2b by outputs, not directory name). Dynamic reranker shapes tested: (B,L)=(1,2),(3,7),(16,48),(32,64), maximum error **0.0000448227** vs Torch.
* fp16 was exported and tested too: maximum v3 logprob error vs fp32 Torch **0.00740290**, maximum vs Core ML **0.0734596**. fp32 remains the baseline. A first onnxconverter-common conversion rejected the pre-existing fp16 memory boundary; forcing it created a mixed-type Mul graph. This was replaced with direct PyTorch half-weight export, retaining int32 inputs, fp16 K/V memory and fp32 logprob. The final exports load and pass all parity checks; the converter dependency is no longer required (its installation remains recorded above).
* The preceding attempt's abandoned Mac matrix was still running under `training/remote_eval/results/frozen`. Its PID/cwd and descendants were verified before stopping only that matrix (PID 72887) and its workers. Its completed artifacts were preserved. Other engineer evaluations and the GPU training service were left running. Wall times are measurements under shared machine load, not isolated hardware benchmarks.
* A final core-check run under heavier load reports **3070/3083, 13 failures**, all latency-only, versus the initial **3072/3083, 11 failures**. The total checks and behavioral passes are unchanged; the latency failure count is not identical under changing load. Exact logs are retained rather than relabeling timing failures as passes.
* An adapter smoke run exposed argparse abbreviation consuming `--cli` as `--cli-arg`; `allow_abbrev=False` fixes that root cause without changing any existing evaluator. The remote smoke suite was restarted after this fix.

### Intermediate whole-engine checks and transport

All five no-model rows are identical on Mac/Linux (0/400 output differences per row). The first reranker rows differ on 1/400 colloquial and 2/400 Wikipedia predictions; chat has 0/400. Accuracies agree at the evaluator's reported precision. These rows are below the 1% target; the final matrix remains the acceptance criterion.

Two long rsync streams were reset by the SSH peer (one initial checkpoint sync, one ONNX retrieval). The complete remote checkpoint copies and source/data manifest were independently verified before evaluation. Subsequent small transfers succeeded. Keepalive options were added to the runner; ONNX retrieval uses resumable partial transfers. This did not stop the remote evaluation process or affect the training workspace.

The final adapter suite now contains **four** tests, including an end-to-end fake-CLI regression for the argparse abbreviation bug. The original evaluator scripts are untouched. `error_analysis`, `typing_errors` and `typo_eval` are smoke-tested remotely with independent batches and four workers; the 400-run matrix uses 60 workers.

### Completed Mac / Linux CPU validation

The complete Cartesian matrix is **15 rows x 400 runs = 6,000 predictions per platform**, including all three datasets under no models, reranker (count 16, weight 1), reranker + v3 (weight 1), and each configuration's Wikipedia `--line-context` / colloquial `--split continue` variants. `compare.py` asserts identical ordered CLI inputs before comparing outputs. Every row passes the <=1% target; the worst row is **3/400 = 0.75%**. Raw predictions, evaluator output and wall time are under `data/build/remote_eval/{mac,remote/linux}/`; the combined table is `data/build/remote_eval/comparison.md`.

Wall times include evaluator preparation, CLI/model startup and typing, with Mac **8** workers and Linux **60**. Shared machine load and startup cost matter: the Linux v3 continued-composition row is slightly slower than the Mac. No hardware-isolated speedup claim is made.

The initial GPU benchmark hit the pre-existing socket implementation's Linux Foundation `read(upToCount:)` behavior: the short reply remained blocked on an open stream until timeout. A Linux-only `recv` branch now consumes available fragments while preserving macOS's original read path. The regression server deliberately holds the socket open; **eight short replies passed**, and a 4-run real GPU smoke passed at 100% whole-run accuracy. The full GPU experiment was restarted once after this fix, with the same frozen engine/data/model weights. Its 2 GiB allocation limit is enforced; observed GPU process memory was around 1.1 GiB at that point.

The CPU validation's exact 43-file original manifests are retained. The only source change after that freeze is the Linux-only optional socket read in `ModelServer.swift`, which is never called by the ONNX CPU path and is compiled out of macOS. Final manifests are recorded separately; no engine or scorer logic changed after the matrix freeze. The Mac all-product release build passed again after that guard was added.

The full `run.sh` integration (12 runs, 12 workers, reranker) passed: sync, Linux release build, content-keyed ONNX export, evaluation and automatic retrieval. Result: **91.7%** whole-run accuracy, 12/12 output lines, with the original evaluator's normal output.

| Config | Set | Mac accuracy | Linux accuracy | Different / runs | Mac seconds | Linux seconds |
|---|---|---:|---:|---:|---:|---:|
| plain | colloquial | 75.5% | 75.5% | 0/400 | 27.28 | 5.08 |
| plain | wikipedia | 71.2% | 71.2% | 0/400 | 21.87 | 4.98 |
| plain | chat | 73.0% | 73.0% | 0/400 | 26.74 | 4.11 |
| plain | context | 71.5% | 71.5% | 0/400 | 35.45 | 4.40 |
| plain | continue | 70.5% | 70.5% | 0/400 | 18.46 | 3.84 |
| reranker | colloquial | 85.0% | 85.0% | 1/400 | 197.37 | 30.55 |
| reranker | wikipedia | 81.5% | 81.5% | 2/400 | 90.63 | 42.31 |
| reranker | chat | 67.8% | 67.8% | 0/400 | 49.76 | 29.45 |
| reranker | context | 84.0% | 84.2% | 3/400 | 181.39 | 115.54 |
| reranker | continue | 79.2% | 79.2% | 1/400 | 46.72 | 25.79 |
| v3 | colloquial | 85.2% | 85.2% | 0/400 | 64.69 | 49.36 |
| v3 | wikipedia | 81.0% | 81.2% | 1/400 | 100.91 | 66.58 |
| v3 | chat | 71.8% | 71.8% | 1/400 | 60.22 | 46.81 |
| v3 | context | 85.2% | 85.5% | 1/400 | 214.08 | 139.71 |
| v3 | continue | 81.0% | 81.0% | 0/400 | 38.76 | 41.62 |

### Final source checks and artifacts

Final source/data manifests still match all 43 files across platforms (`manifest-final-{mac,linux}.json`). Their only difference from the original matrix manifests is the optional Linux socket transport guard. The retrieved local baseline ONNX models and v3 fp16 experiments have SHA256 hashes identical to the remote exports; hashes are retained in `data/build/remote_eval/model-sha256.txt`.

After the Mac matrix finished, checks reported **3076/3083, seven latency-only failures**, all a subset of the fresh baseline's 11 failures. There are **no new behavioral failures**. The intermediate, more heavily loaded check run had 13 timing failures. The requested identical timing failure count could not be reproduced under changing shared load; no performance thresholds or tests were weakened. Final log: `data/build/remote_eval/checks-after-matrix.log`.

Full measured CPU matrix totals: **Mac 1174.31 s; Linux 610.12 s** (1.92x faster for this complete matrix under observed shared load). All 6,000 inputs align exactly; **10/6,000 output predictions differ**, with the worst individual row 3/400. This is a platform comparison, not evidence that the macOS backend became slower: the Mac retains Core ML and its existing inference path.

Linux core checks also ran: **3075/3081, six latency-only failures**, with no behavioral failure reported. Two checks for the optional Mac `data/build/lm2` fixture are absent because that unshipped model is not part of the remote sync. The existing timing targets were not changed.

Linkage was verified: the Mac CLI links CoreML.framework and has no ONNX dependency; the Linux CLI resolves `libonnxruntime.so.1` from `~/opt/onnxruntime/lib/`. All six exported ONNX files (three fp32, three fp16) are now retrieved alongside the Mac model JSON and match remote SHA256 hashes.

Runtime references: [ONNX Runtime C API](https://onnxruntime.ai/docs/get-started/with-c.html), [thread management](https://onnxruntime.ai/docs/performance/tune-performance/threading.html), [official 1.24.4 SDK release](https://github.com/microsoft/onnxruntime/releases/tag/v1.24.4). The measured results above come from this workspace's captures, not those reference pages.

### Completed GPU comparison and selected options

After the socket fix, the complete 15-row GPU matrix took **393.13 s**, versus **610.12 s** for ONNX CPU: **1.55x faster / 35.6% less row wall time**. All **6,000 GPU predictions equal the CPU predictions**, so the Mac differences and accuracies are identical to the CPU table above. Raw GPU predictions and timings are `data/build/remote_eval/remote/gpu/`; `comparison-gpu.md` retains the Mac/GPU table. The GPU was fp32 with TF32 disabled, not bf16. The optional server uses PyTorch checkpoint inference; native per-process Linux model inference is ONNX Runtime through the C API.

This is enough improvement to expose **`BACKEND=cuda`** in run.sh for large remote sweeps. **`BACKEND=cpu` stays the default** and requires no GPU. Both modes build the same Swift engine/CLI. The CUDA mode owns a private socket/server, has a 2 GiB Torch allocation cap, propagates model/server errors, and terminates only its own server on exit. Observed GPU process use was approximately **0.8–1.2 GiB**, alongside the unchanged existing VLLM process (~43.5 GiB). No ONNX CUDA SDK or driver installation was needed.

The matrix totals are sums of per-row evaluator wall times; the shared GPU server's one-time startup/cleanup is excluded (its Python initialization/import/load is a few seconds, not charged per row). CPU timings include model startup in each CLI worker. Source synchronization, build and ONNX export are excluded from both steady evaluation totals. All timings were measured under concurrent lab activity.

| Configuration | Set | CPU seconds | GPU seconds |
|---|---|---:|---:|
| plain | colloquial | 5.08 | 4.05 |
| plain | wikipedia | 4.98 | 4.46 |
| plain | chat | 4.11 | 3.93 |
| plain | context | 4.40 | 4.52 |
| plain | continue | 3.84 | 3.77 |
| reranker | colloquial | 30.55 | 21.59 |
| reranker | wikipedia | 42.31 | 26.37 |
| reranker | chat | 29.45 | 21.02 |
| reranker | context | 115.54 | 38.25 |
| reranker | continue | 25.79 | 18.41 |
| v3 | colloquial | 49.36 | 35.27 |
| v3 | wikipedia | 66.58 | 51.25 |
| v3 | chat | 46.81 | 35.18 |
| v3 | context | 139.71 | 95.22 |
| v3 | continue | 41.62 | 29.83 |

### Final runner verification and handoff

`BACKEND=cuda JOBS=4 training/remote_eval/run.sh sweep --matrix training/remote_eval/validation.json -n 4 --output data/build/remote_eval/runner-gpu` passed all **15** small integration rows. This verifies the new global socket flag through sweep.py, remote server startup, the same line-context/split variants, source sync including the test target, build/export-cache reuse, JSON/table retrieval and exit cleanup. The earlier ONNX CPU run.sh integration also passed. The final Python regression suite is **4/4**, scorer tests **2/2** on each OS, and socket regression **8 requests**. Shell syntax checks passed.

All task-owned evaluation servers were stopped on exit. A final GPU process inventory shows only the original VLLM PID 262190 at 43547 MiB. No native Linux/Windows input-method integration, C ABI implementation, DirectML/OpenVINO provider, model quantization or app installation was attempted; their boundaries and packaging plan are in PORTING.md. The existing macOS Core ML runtime and app/front-end files were not changed by this task. Existing unrelated dirty working-tree changes remain untouched. No git commit/stash/reset/checkout was performed.

Acceptance summary: portable release builds pass; fp32/fp16 model checks pass; all 15 CPU and 15 GPU Mac comparisons pass the 1% output-difference target; both remote runner modes pass integration; remaining core-check failures are pre-existing latency thresholds under shared load (Mac final 7 vs fresh baseline 11; Linux 6). The failure counts are reported as measured rather than claiming exact stability under load.
