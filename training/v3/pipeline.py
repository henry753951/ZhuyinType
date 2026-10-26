"""Remote nohup stage runner. Stops at the first failure; does not retry guesses."""
import json
import argparse
import os
from pathlib import Path
import subprocess
import sys
import time

from common import OUT, ROOT, save


def stage(name, script, *args):
    status = {"stage": name, "status": "running", "started": time.time()}
    save(OUT / "status.json", status)
    path = ROOT / "logs" / f"v3-{name}.log"
    with path.open("w") as log:
        command = [sys.executable, "-u", str(ROOT / "training/v3" / script), *args]
        result = subprocess.run(command, cwd=ROOT, stdout=log, stderr=subprocess.STDOUT,
                                env={**os.environ, "PYTHONDONTWRITEBYTECODE": "1", "OPENBLAS_NUM_THREADS": "1", "OMP_NUM_THREADS": "4"})
    status.update(exit=result.returncode, seconds=time.time() - status["started"], status="complete" if result.returncode == 0 else "failed")
    save(OUT / f"stage-{name}.json", status)
    save(OUT / "status.json", status)
    if result.returncode:
        raise RuntimeError(f"{name} failed, exit {result.returncode}; see {path}")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--from-syllable", action="store_true", help="Recover after context training completed")
    args = parser.parse_args()
    if args.from_syllable:
        assert (OUT / "context-done.json").exists() and (OUT / "context.pt").exists()
    else:
        prepare_and_context()
    stage("syllable", "train.py", "syllable", "--steps", "60000", "--batch", "64")
    done = json.load(open(OUT / "syllable-done.json"))
    while not done["plateau"]:
        previous_step = done["steps"]
        stage(f"syllable-continue-{previous_step}", "train.py", "syllable", "--resume", "--steps", "10000", "--batch", "64")
        done = json.load(open(OUT / "syllable-done.json"))
        assert done["steps"] > previous_step, "continuation did not advance training"
    stage("quality", "evaluate.py")
    stage("export", "export.py")
    save(OUT / "status.json", {"stage": "remote complete", "status": "awaiting Mac compile/parity/vectors/latency"})


def prepare_and_context():
    # These two independent extraction jobs were launched first while code was prepared.
    for name in ["wiki", "colloquial"]:
        while not (OUT / f"{name}.json").exists():
            pattern = f"[p]ython -u training/v3/prepare.py {name}"
            if subprocess.run(["pgrep", "-f", pattern], stdout=subprocess.DEVNULL).returncode:
                raise RuntimeError(f"{name} producer exited without completion marker; see logs/v3-{name}.log")
            save(OUT / "status.json", {"stage": name, "status": "waiting for preparation"})
            time.sleep(30)
    stage("vocab", "prepare.py", "vocab")
    stage("polyphone", "readings.py")
    stage("encode", "encode.py")
    stage("data-audit", "verify_data.py")
    stage("selftest", "test_v3.py")
    stage("context", "train.py", "context", "--steps", "30000", "--batch", "64")


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        save(OUT / "status.json", {"stage": "stopped", "status": "failed", "error": str(exc)})
        raise
