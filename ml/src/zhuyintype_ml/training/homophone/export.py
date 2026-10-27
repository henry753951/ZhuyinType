"""Export the chosen continuation on the Mac: Core ML packages, compiled models, golden vectors, notices.

    zt train homophone export                                 # macOS only
    python -m zhuyintype_ml.training.homophone.export [--model data/build/v3_homophone2]

Only the checkpoint sealed in homophone2/choice2.json is exported.  Uses the v3 exporter unchanged (fp16, macOS 14),
sets the package licence to point at NOTICE.md, compiles both packages, and writes test_vectors.json: the Core ML
outputs for the 20 interface cases of the original v3 (v3_030/test_vectors.json), so zhuyin-cli --check-v3 can
compare against them.  Copies the lexicon, articles.tsv.gz and example.json beside the model, and writes NOTICE.md
with every source's licence, the lm_chat2 attribution and source manifest, the Qwen3-8B licence and the app's
component licences.  The result is what ml/src/zhuyintype_ml/data/assets.py ships as v3/.
"""

from __future__ import annotations

import argparse
import json
import shutil
from pathlib import Path

from zhuyintype_ml.core import paths
from zhuyintype_ml.training.v3.coreml import compile_package, require_macos

from .common import BASE, MODEL, RUN, digest, save

NOTICE = (
    "# ZhuyinType compatible homophone continuation\n\n"
    "Model and derived corpus: CC BY-SA 4.0, https://creativecommons.org/licenses/by-sa/4.0/ . Code: AGPL-3.0-only.\n\n"
    "Chinese Wikipedia contributors: article paragraphs and talk-page revisions, CC BY-SA 4.0. "
    "Retain articles.tsv.gz and attribution.jsonl (revision/history links). "
    "https://foundation.wikimedia.org/wiki/Policy:Terms_of_Use#7._Licensing_of_Content\n\n"
    "Legislative Yuan, Taiwan, 2024–2025 gazettes: OGDL v1, https://data.gov.tw/license . "
    "Via OpenFun LYAPI, CC BY 4.0, https://v2.ly.govapi.tw/ . "
    "This open data is supplied by the Legislative Yuan under the Open Government Data License. "
    "Original document identities and URLs are retained in attribution.jsonl.\n\n"
    "Common Voice sentence-collector contributors: CC0 1.0, commit "
    "20fd76cd7d3b1e937fe99359b2ef3649b9c466ac. "
    "https://creativecommons.org/publicdomain/zero/1.0/\n\n"
    "Deduplicated existing colloquial corpus: Qwen3-8B-generated sentences and Common Voice. "
    "Qwen3-8B weights: Apache-2.0 (Qwen3-8B-LICENSE); generated text has no additional output-use restriction.\n\n"
    "Seed lexicon: CC-CEDICT CC BY-SA 4.0; Unihan Unicode License v3; wordfreq data CC BY-SA 4.0. "
    "Preserve component notices.\n\n"
    "Changes: continue original v3 with heldout 8-character/context exclusions, real-source supplement, "
    "balanced sentence-initial sampling and 3x confusion-position loss. "
    "See ml/src/zhuyintype_ml/training/homophone/REPORT.md for validation and limitations. "
    "Retain this notice, source-manifest.json, attribution.jsonl, articles.tsv.gz and component notices. "
    "No source organization endorses this model.\n"
)
LICENSE = "See NOTICE.md: CC BY-SA 4.0, CC BY 4.0, OGDL v1, CC0 1.0 and component terms."


def export(target: Path, run: Path) -> None:
    require_macos("the homophone export")
    choice = json.loads((run / "choice2.json").read_text())
    assert digest(target / "syllable.pt") == choice["checkpoint"], "Export only the dev winner."
    shutil.copy2(paths.data("lexicon.tsv"), target / "lexicon.tsv")
    shutil.copy2(BASE / "articles.tsv.gz", target / "articles.tsv.gz")
    shutil.copy2(BASE / "example.json", target / "example.json")
    import coremltools as ct
    import numpy as np

    from zhuyintype_ml.training.v3.export import export as export_v3
    from zhuyintype_ml.training.v3.validate import tensor

    export_v3(target / "syllable.pt", target)
    models = []
    for name in ["ContextEncoder", "SyllableEncoder"]:
        package = target / f"{name}.mlpackage"
        model = ct.models.MLModel(str(package), skip_model_load=True)
        model.license = LICENSE
        licensed = target / f"{name}.licensed.mlpackage"
        model.save(str(licensed))
        shutil.rmtree(package)
        licensed.rename(package)
        compile_package(package, target)
        models.append(ct.models.MLModel(str(package), compute_units=ct.ComputeUnit.CPU_ONLY))
    vectors = []
    # Golden outputs for the unchanged interface cases; no corpus evaluation on the Mac.
    template = json.loads((BASE / "test_vectors.json").read_text())
    for original in template["vectors"]:
        values = {n: np.asarray(v, np.int32) for n, v in original["inputs"].items()}
        memory = models[0].predict({"context_ids": values.pop("context_ids")})
        score = models[1].predict({**values, **memory})["logprob"]
        assert score.shape == values["candidate_ids"].shape and np.isfinite(score).all()
        np.testing.assert_allclose(np.exp(score).sum(-1), 1, atol=0.01, rtol=0.01)
        vectors.append(
            {**original, "expected": {**{n: tensor(v) for n, v in memory.items()}, "logprob": score.tolist()}}
        )
    save(
        target / "test_vectors.json",
        dict(
            version=1,
            backend="Core ML CPU_ONLY",
            vectors=vectors,
            memory_format=template["memory_format"],
            comparison=dict(coreml_logprob_atol=0.002, coreml_logprob_rtol=0.002, pytorch_fp32_atol=0.1),
            package_sha256={
                str(p.relative_to(target)): digest(p)
                for name in ["ContextEncoder", "SyllableEncoder"]
                for p in (target / f"{name}.mlpackage").rglob("*")
                if p.is_file()
            },
        ),
    )
    (target / "NOTICE.md").write_text(NOTICE)
    for name in ["attribution.jsonl", "source-manifest.json"]:
        shutil.copy2(paths.data("lm_chat2", name), target / name)
    shutil.copy2(paths.data("corpus", "raw", "Qwen3-8B-LICENSE"), target / "Qwen3-8B-LICENSE")
    shutil.copytree(
        paths.ROOT / "platforms/macos/Resources/Licenses",
        target / "Licenses",
        dirs_exist_ok=True,
        ignore=shutil.ignore_patterns("*Taiwan*", "*Tongues*"),
    )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--model", type=Path, default=MODEL, help="model directory (default: data/build/v3_homophone2)")
    parser.add_argument("--run", type=Path, default=RUN, help="run directory with choice2.json (default: homophone2)")
    args = parser.parse_args(argv)
    export(args.model, args.run)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
