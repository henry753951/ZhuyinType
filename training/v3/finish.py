"""Monitor remote jobs, maintain REPORT.md, then retrieve/compile/validate on Mac."""
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import shlex
import subprocess
import sys
import time

from common import ROOT, OUT, save

SSH = ["ssh", "-o", "BatchMode=yes", "-o", "ConnectTimeout=15", "-i", str(Path.home() / ".ssh/<remote-key>"), "$ZHUYIN_REMOTE_HOST"]
REMOTE = "~/zhuyintype"
REPORT = ROOT / "training/v3/REPORT.md"


def probe():
    code = r'''
import json
from pathlib import Path
import subprocess
p=Path('~/zhuyintype'); out=p/'data/build/v3'
result={'metrics':{}}
for name in ['status','wiki','colloquial','provenance','polyphone','encode','data-audit','context-done','syllable-done','context-history','syllable-history','quality']:
 f=out/(name+'.json')
 if f.exists(): result['metrics'][name]=json.loads(f.read_text())
stage=result['metrics'].get('status',{}).get('stage','wiki')
f=p/'logs'/('v3-'+stage+'.log')
if f.exists():
 with f.open('rb') as h:
  h.seek(max(0,f.stat().st_size-4096)); result['tail']=h.read().decode(errors='replace').splitlines()[-3:]
proc=subprocess.run(['pgrep','-f','[p]ython -u .*training/v3/|[p]ython -u training/v3/'],capture_output=True,text=True)
result['pids']=proc.stdout.split()
print(json.dumps(result))
'''
    result = subprocess.run(SSH + [shlex.join([REMOTE + "/.venv-gen/bin/python", "-c", code])], capture_output=True, text=True)
    if result.returncode:
        raise RuntimeError(f"Remote unreachable/probe failed (exit {result.returncode}): {result.stderr.strip()}")
    return json.loads(result.stdout)


def report(snapshot, final=None):
    prefix = REPORT.read_text().split("## Live execution", 1)[0].rstrip()
    metrics = snapshot.get("metrics", {})
    status = metrics.get("status", {})
    rows = ["", "## Live execution", "", f"Updated UTC: {datetime.now(timezone.utc).isoformat()}", "",
            f"Status: {final or status}.", "", "Remote jobs use nohup, per-stage `logs/v3-*.log`, and stop at first failure. No automatic retries."]
    if snapshot.get("tail"):
        rows += ["", "Latest remote log:", "", "```text", *snapshot["tail"], "```"]
    if "wiki" in metrics:
        w = metrics["wiki"]
        rows += ["", f"Wikipedia: **{w['paragraphs']:,} paragraphs**, {w['characters']:,} characters; "
                 f"{w['articles_kept']:,} articles; {w['excluded_paragraphs']:,} contaminated paragraphs removed; {w['seconds']/60:.2f} minutes."]
    if "colloquial" in metrics:
        c = metrics["colloquial"]
        rows += ["", f"Colloquial: {c['input_lines']:,} inputs → **{c['kept']:,} kept**; normalized duplicates "
                 f"{c.get('normalized_duplicates',0):,}, near duplicates {c.get('near_duplicates',0):,}, family cap removals "
                 f"{c.get('family_cap_removed',0):,}, heldout/empty exclusions {c.get('excluded',0):,}; "
                 f"{c['seconds']/60:.2f} minutes. Family maximum {c['max_family']} (cap {c['family_cap']}).", "", c["limit"] + "."]
    if "provenance" in metrics:
        p = metrics["provenance"]
        rows += ["", f"Character vocabulary {p['vocab']:,}; single-character lexicon entries occurring in corpus "
                 f"{p['single_character_lexicon_in_corpus']:,}. Vocabulary also retains observed punctuation, Latin and other corpus characters."]
    if "polyphone" in metrics:
        p = metrics["polyphone"]
        rows += ["", f"Polyphone classifier: {p['examples']:,} training-only word-supervised examples; "
                 f"{p['reading_classes']} classes, {p['supervised_pairs']} character/reading pairs; {p['seconds']/60:.2f} minutes. "
                 "9-character window, 48-dimensional embeddings, 192-hidden-unit MLP. Prediction is normalized over the central character's lexicon readings; p>=0.9 required.",
                 "", "Internal seeded 10% validation (not an independent annotated G2P benchmark):", "", "```json", json.dumps(p["history"][-1], indent=2), "```"]
    if "encode" in metrics:
        e = metrics["encode"]
        rows += ["", "Encoded corpus:", "", "```json", json.dumps(e["totals"], indent=2), "```", "",
                 f"Encoding time: {e['seconds']/60:.2f} minutes. Low-confidence examples are excluded from syllable training but retained for the context LM. "
                 "Colloquial context is the preceding retained line; existing generated lines are independent messages, not genuine dialogue."]
    if "data-audit" in metrics:
        a = metrics["data-audit"]
        rows += ["", f"Full post-extraction leakage audit passed for wiki and colloquial; "
                 f"{len(a['samples'])} encoded reading/context samples checked. Details: data/build/v3/data-audit.json."]
    rows += ["", "| Model | Parameters | Steps | Selected step | Minutes | Selected dev NLL | Plateau reached |", "|---|---:|---:|---:|---:|---:|---|"]
    for name in ["context", "syllable"]:
        d = metrics.get(name + "-done")
        if d:
            rows.append(f"| {name} | {d['parameter_count']:,} | {d['steps']} | {d['selected_step']} | {d['seconds']/60:.2f} | {d['best_dev_nll']:.4f} | {d['plateau']} |")
        elif name + "-history" in metrics:
            d = metrics[name + "-history"]["history"][-1]
            rows.append(f"| {name} (running) | — | {d['step']} | — | {d['minutes']:.2f} | {metrics[name+'-history']['best_nll']:.4f} | pending |")
    rows += ["", "Both models: 6 layers, 512 width, 8 heads, 2048 FFN, pre-LN and RoPE. "
             "Context LM uses 256 positions and a tied character head; syllable self-attention/character embeddings initialize from it. "
             "The context encoder is frozen during syllable training; each syllable layer's cross K/V projections train and are copied into ContextEncoder export. "
             "Context drops for 30% of training batches. Wiki and colloquial shards are sampled by token/example count, with colloquial weighted twice. "
             "Validation every 1000 steps; improvement threshold 0.002 NLL; patience 4 after at least 5000 initial steps. "
             "Initial segment limits are 30000/60000 steps; syllable continuation uses 10000-step segments until plateau."]
    if "quality" in metrics:
        q = metrics["quality"]
        rows += ["", "### Offline dev selection accuracy", "", "Backend: PyTorch CUDA BF16. Core ML CPU parity is measured separately below.", "", q["protocol"], "",
                 "| Dev | Context | Requested noise | Positions | Model top-1 % | Lexicon baseline % | Valid changed % | Candidates >64 |",
                 "|---|---|---:|---:|---:|---:|---:|---:|"]
        for d in q["rows"]:
            rows.append(f"| {d['domain']} | {d['line_context']} | {100*d['requested_noise']:.0f}% | {d['positions']} | "
                        f"{100*d['model_top1']:.2f} | {100*d['baseline_top1']:.2f} | {100*d['actual_valid_noise_fraction']:.2f} | {d['candidate_count_over64']} |")
        rows += ["", "Baseline uses the highest-prior **single-character** lexicon row for the typed reading. "
                 "Training and offline selection evaluate the full candidate union, including gold; exported C<=64 requires caller pruning. "
                 "This evaluates character selection conditional on gold recall, not the Swift decoder, whole-sentence correctness, or M4 engine adoption gate."]
    latency_path = OUT / "latency.json"
    if latency_path.exists():
        l = json.load(latency_path.open())
        rows += ["", "### Core ML CPU-only latency", "", f"Machine: {l['cpu']}; {l['machine']}. {l['protocol']}.", "",
                 "| Model | Length | p50 ms | p95 ms |", "|---|---:|---:|---:|"]
        for name in ["context", "syllable"]:
            for length, d in l[name].items():
                rows.append(f"| {name} | {length} | {d['p50_ms']:.3f} | {d['p95_ms']:.3f} |")
        rows += ["", f"Syllable Ls=30, C=32, Lc=256 target <=15 ms: p50 {'PASS' if l['target_30_p50_le15ms'] else 'FAIL'}, "
                 f"p95 {'PASS' if l['target_30_p95_le15ms'] else 'FAIL'}."]
    parity_path = OUT / "parity.json"
    if parity_path.exists():
        parity = json.load(parity_path.open())
        positions = sum(d['syllables'] for d in parity)
        changes = sum(round((1 - d['top1_agreement']) * d['syllables']) for d in parity)
        rows += ["", f"20 Core ML golden vectors passed shape/finite/normalization/parity checks. "
                 f"FP16 Core ML changed {changes}/{positions} argmax positions versus fp32 PyTorch on these synthetic interface cases. Maximum fp32 PyTorch logprob "
                 f"difference {max(d['max_logprob_abs_error'] for d in parity):.6f}; memory difference {max(d['max_memory_abs_error'] for d in parity):.6f}. "
                 "test_vectors.json contains exact CPU Core ML outputs, with float16 memories encoded as base64 little-endian bytes and SHA256. "
                 "Core ML comparisons use atol/rtol 0.002; fp32 PyTorch uses atol 0.1. Both mlpackages compiled with the requested Xcode coremlcompiler."]
    rows += ["", "### Limits", "", "No Swift changes, installation, engine integration, decoder recall, end-to-end UI tests, optional dialogue generation, int8 export, or held-out test scoring. "
             "Existing independent colloquial messages provide sequential context without conversational guarantees. "
             "Polyphone confidence relies on word-derived labels and has not been human-calibrated. "
             "Core ML CPU target is reported as measured, not assumed. Models are not promoted to app resources."]
    REPORT.write_text(prefix + "\n" + "\n".join(rows) + "\n")


def command(args, log):
    with (OUT / log).open("w") as stream:
        subprocess.run(args, cwd=ROOT, stdout=stream, stderr=subprocess.STDOUT,
                       env={**os.environ, "PYTHONDONTWRITEBYTECODE": "1", "DEVELOPER_DIR": "/Applications/Xcode.app/Contents/Developer"}, check=True)


def main():
    while True:
        snapshot = probe()
        save(OUT / "remote-progress.json", snapshot)
        report(snapshot)
        status = snapshot["metrics"]["status"]
        print(datetime.now().isoformat(), status, snapshot.get("tail", []), flush=True)
        if status.get("status") == "failed":
            raise RuntimeError(str(status))
        if status.get("stage") == "remote complete":
            break
        if not snapshot["pids"]:
            raise RuntimeError("Remote v3 processes exited without completion status")
        time.sleep(30)
    files = ["chars.json", "syllables.json", "lexicon.tsv", "syllable.pt", "context.pt", "polyphone.pt",
             "ContextEncoder.mlpackage", "SyllableEncoder.mlpackage", "config.json", "articles.tsv.gz"]
    files += [p + ".json" for p in snapshot["metrics"] if p not in ["status"]]
    for name in dict.fromkeys(files):
        command(["rsync", "-a", "-e", shlex.join(SSH[:-1]), SSH[-1] + ":" + REMOTE + "/data/build/v3/" + name, str(OUT) + "/"], "transfer.log")
    # Export again using the pinned Mac Torch version to make the handoff reproducible on this machine.
    command([sys.executable, "training/v3/export.py"], "export-mac.log")
    for name in ["ContextEncoder", "SyllableEncoder"]:
        command(["xcrun", "coremlcompiler", "compile", str(OUT / (name + ".mlpackage")), str(OUT)], name + "-compile.log")
    command([sys.executable, "training/v3/validate.py"], "validate.log")
    report(snapshot, "Complete: trained, exported, compiled and measured; see gates and limitations below")
    save(OUT / "complete.json", {"status": "complete", "utc": datetime.now(timezone.utc).isoformat()})


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        snapshot_path = OUT / "remote-progress.json"
        snapshot = json.loads(snapshot_path.read_text()) if snapshot_path.exists() else {}
        report(snapshot, f"STOPPED after failure: {type(exc).__name__}: {exc}. No results guessed.")
        save(OUT / "failure.json", {"error": str(exc), "utc": datetime.now(timezone.utc).isoformat()})
        raise
