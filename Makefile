# ZhuyinType: the shared engine (nlp/engine), its data and evaluation (nlp/, training/), and the macOS client
# (clients/macos).  Every target runs from the repository root.
.PHONY: lexicon data app debug-app install pkg checks cli clean

lexicon:            ## build or reuse data/build/lexicon.tsv
	./nlp/data/build-lexicon.sh
data:               ## compile lexicon and language models into mmap binaries (OUT=dir)
	./nlp/data/compile-data.sh $${OUT:-data/build/compiled}
cli:                ## zhuyin-cli and the engine checks
	swift build --package-path nlp/engine --scratch-path "$${SWIFT_SCRATCH_PATH:-.build}" -c release
checks:             ## engine behaviour and latency checks
	swift run --package-path nlp/engine --scratch-path "$${SWIFT_SCRATCH_PATH:-.build}" -c release ZhuyinCoreChecks
app:                ## dist/ZhuyinType.app
	./clients/macos/scripts/build-app.sh release
debug-app:
	./clients/macos/scripts/build-app.sh debug
install:            ## install into ~/Library/Input Methods (the running copy switches over when idle)
	./clients/macos/scripts/install-dev.sh
pkg:                ## dist/ZhuyinType-<version>.pkg
	./clients/macos/scripts/build-pkg.sh
clean:
	rm -rf dist .build .build-pkg
