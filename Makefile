.PHONY: lexicon app debug-app install pkg checks clean

lexicon:
	./scripts/build-lexicon.sh
app:
	./scripts/build-app.sh release
debug-app:
	./scripts/build-app.sh debug
install:
	./scripts/install-dev.sh
pkg:
	./scripts/build-pkg.sh
checks:
	swift run --scratch-path "$${SWIFT_SCRATCH_PATH:-.build}" -c release ZhuyinCoreChecks
clean:
	rm -rf dist .build .build-pkg
