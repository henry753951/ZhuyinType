# ZhuyinType: the shared engine (engine/, Rust), its data, training and evaluation (ml/), and the macOS client
# (platforms/macos).  Every target runs from the repository root and calls tools/zt (`tools/zt help` lists more).
.PHONY: setup fetch lexicon data app debug-app install pkg checks cli test lint types clean

setup:              ## check tools, prepare the uv environment
	tools/zt setup
fetch:              ## download the pinned data and models into data/build
	tools/zt data fetch
lexicon:            ## build or reuse data/build/lexicon.tsv
	tools/zt data lexicon
data:               ## compile lexicon and language models into mmap binaries (OUT=dir)
	tools/zt data compile $${OUT:-data/build/compiled}
cli:                ## zhuyin-cli (engine/target/release)
	tools/zt build cli
checks:             ## engine behaviour and latency checks (cargo test --release)
	tools/zt check engine
app:                ## dist/ZhuyinType.app
	tools/zt build app
debug-app:
	tools/zt build app --debug
install:            ## install into ~/Library/Input Methods (the running copy switches over when idle)
	tools/zt build install
pkg:                ## dist/ZhuyinType-<version>.pkg
	tools/zt build pkg
test:               ## Python unit tests (ml/src/**/test_*.py)
	tools/zt check python
lint:               ## ruff
	tools/zt check lint
types:              ## mypy
	tools/zt check types
clean:
	rm -rf dist .build .build-pkg engine/target
