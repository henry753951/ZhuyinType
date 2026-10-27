# Working on ZhuyinType (for agents and contributors)

ZhuyinType is an open-source Zhuyin (Bopomofo) input method. The shared engine and everything about choosing
characters live in `nlp/`; each platform's client lives in `clients/` (macOS today). Read this file first, then the
document for the area you touch.

## Layout

| Path | What | Docs |
|---|---|---|
| `nlp/engine/` | Swift package, cross-platform: `ZhuyinCore` (composer, key decoder, typo channel, lexicon, language models, personal model), `ZhuyinML` (sentence reranker v2b and syllable-to-character model v3: Core ML on macOS, ONNX Runtime on Linux via `ONNXBridge`), `zhuyin-cli` (every evaluation goes through it), `ZhuyinCoreChecks` | `docs/ARCHITECTURE.md`, `docs/MODEL.md` |
| `nlp/data/` | Lexicon builder, data compiler (mmap binaries), asset manifest (`assets.json`, `assets.py`), data sources | `nlp/data/SOURCES.md` |
| `nlp/training/` | One directory per model or data pipeline (`lm`, `lm_chat`, `lm_chat2`, `reranker`, `v3`, `homophone`, `base`, `corpus`, `memory`), each with README/REPORT | `nlp/training/README.md` |
| `nlp/eval/` | Evaluation tools (conversion, candidates, repick, typing habits, external rows, paired statistics, Mac metrics) and ZhuyinBench (`bench/`) | `docs/EVALUATION.md` |
| `nlp/remote/` | The GPU-host runner: syncs the engine, builds it on Linux, serves the models over ONNX Runtime and runs any `nlp/eval` tool there | `nlp/remote/README.md` |
| `clients/macos/` | Swift package for the InputMethodKit app (depends on `../../nlp/engine` and Sparkle), Resources, build/release scripts | `docs/DESIGN.md` |
| `docs/` | Design, architecture, models, evaluation, roadmap, research (`docs/research/`), per-version benchmarks (`docs/benchmarks/`), release notes (`docs/releases/`) | `docs/ROADMAP.md` |
| `data/` | Local only (gitignored): `data/build/` holds data and models fetched by `nlp/data/assets.py` or produced by training | |
| `references/` | Local only: other IMEs for study. Never copy code from them (0c is AGPL-3.0) | `docs/research/README.md` |

## Commands (run from the repository root)

```sh
python3 nlp/data/assets.py fetch           # data and models into data/build (pinned by nlp/data/assets.json)
make cli                                    # zhuyin-cli and checks (nlp/engine)
make checks                                 # engine behaviour + latency checks (ZHUYIN_CHECKS_TIMING=warn on slow machines)
make app / make install                     # dist/ZhuyinType.app / install into ~/Library/Input Methods
dist/ZhuyinType.app/Contents/MacOS/ZhuyinType --self-test --lexicon data/build/lexicon.tsv
nlp/remote/run.sh conversion_eval -n 400 ...   # any evaluation, on the GPU host (needs .remote.env)
nlp/eval/bench/run.sh vX.Y.Z           # ZhuyinBench (on the GPU host; CI runs it after each release)
clients/macos/scripts/release.sh            # what the release workflow runs (pkg, Sparkle zip, deltas, appcast)
tools/check-public.sh                       # before pushing: no private hosts, keys or private-network addresses
```

Builds need full Xcode (SwiftUI macros); `build-app.sh` switches `DEVELOPER_DIR` to Xcode automatically.

## Rules

- **Evaluate and train on the GPU host, not on a developer Mac.** The Mac only builds, exports/compiles Core ML,
  measures latency/energy (`nlp/eval/mac_metrics.py`) and does the one-off check that remote results match.
- **Decide on dev sets; test sets only track.** Any adopted change needs a paired comparison (`nlp/eval/stats.py`)
  and an entry in `docs/research/log.md`; tried-and-rejected ideas go there and to `docs/MODEL.md` §11.
- **Keep docs true to the code.** When behaviour, defaults, file formats or numbers change, update `docs/` in the same
  change (ARCHITECTURE for flow, MODEL for methods and numbers, DESIGN for what users see, ROADMAP for status).
- **Releases:** bump `CFBundleShortVersionString`/`CFBundleVersion` in `clients/macos/Resources/Info.plist`, write
  `docs/releases/vX.Y.Z.md` (changes, benchmark summary vs the previous version), commit, then push the tag `vX.Y.Z`.
  CI builds, signs, publishes, updates `appcast.xml`, and starts ZhuyinBench.
- **Data and models are not in git.** Pack and publish new versions with `nlp/data/assets.py pack data-N` and
  `publish data-N`, then commit `nlp/data/assets.json`.
- **Privacy:** no private hosts or keys in tracked files (`.remote.env` is local); no user text in logs (log app
  bundle IDs only as `.private`); secure input pauses context reading and learning.
- **UI copy** follows `docs/WRITING.md` (Taiwan Traditional Chinese, plain, no slogans).
- Long jobs on the GPU host: `nohup`, logs in `~/zhuyintype/logs/`; never kill processes by broad name patterns.
- Commits end with the attribution lines the session asks for; never force-push `main`.
