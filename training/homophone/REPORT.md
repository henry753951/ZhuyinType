# Homophone experiment

Status: in progress. CLI frozen before work, SHA256
`ba2e70306b8d1863a6e68dabc522083d91f3a2301db7586aff3cf9b0479b1fb9`.

Only the requested directories and remote `logs/homophone-*.log` are written.
No engine/source/docs changes, dependency installs, rebuilds or Git mutations.
Existing uncommitted work is preserved.

Remote: H100L-3-47C MIG, 46,587 MiB; initially idle. Reuse Python 3.12.15,
Torch 2.13.0+cu130, coremltools 9.0, NumPy 2.3.5. Mac reuses the pinned v3 environment.

Four regression checks passed: exclusions (including short smoke strings), edit
alignment, annotation categories, and confusion-versus-whole-sentence metrics.
The existing lexicon reader emits an unclosed-file ResourceWarning under unittest;
its code is read-only and was not changed.

The original checkpoints were trained before these mined homophone holdouts existed.
They may already have seen the new evaluation sentences. This experiment guarantees
exclusion from continuation, not retroactive removal from pretraining. This limit
must accompany any reported comparison.
