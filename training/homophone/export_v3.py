"""Final Mac export: compile packages and emit matching portable golden vectors."""
import importlib
import json
import os
import shutil
import subprocess
import sys
import argparse

from common import BASE, OUT, ROOT, digest, save, v3


def main():
    assert sys.platform=='darwin', 'Core ML export requires the Mac.'
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--compatible',action='store_true')
    parser.add_argument('--run-name',choices=['homophone2','homophone2b','homophone2_stable'],default='homophone2')
    args=parser.parse_args()
    target = ROOT/('data/build/v3_'+args.run_name if args.compatible else 'data/build/v3_homophone')
    if args.compatible:
        choice=json.loads((ROOT/'data/build'/args.run_name/'choice2.json').read_text())
        assert digest(target/'syllable.pt')==choice['checkpoint'], 'Export only the dev winner.'
    else:
        assert 'v3_homophone' in json.loads((OUT/'choice.json').read_text())['models'], 'Export only the dev winner.'
    shutil.copy2((ROOT/'data/build/lexicon.tsv') if args.compatible else OUT/'lexicon.tsv',target/'lexicon.tsv')
    shutil.copy2(BASE/'articles.tsv.gz',target/'articles.tsv.gz')
    shutil.copy2(BASE/'example.json',target/'example.json')
    v3()
    import common
    common.OUT = target
    export = importlib.import_module('export')
    export.export(target/'syllable.pt',target)
    import coremltools as ct
    import numpy as np
    from validate import tensor
    models = []
    for name in ['ContextEncoder','SyllableEncoder']:
        package = target/f'{name}.mlpackage'
        model = ct.models.MLModel(str(package),skip_model_load=True)
        model.license = ('See NOTICE.md: CC BY-SA 4.0, CC BY 4.0, OGDL v1, CC0 1.0 and component terms.' if args.compatible else
                         'See NOTICE.md: CC BY-SA 4.0, component terms, and Taiwan Tongues TRAIL v0.1.')
        licensed = target/f'{name}.licensed.mlpackage'
        model.save(str(licensed))
        shutil.rmtree(package)
        licensed.rename(package)
        subprocess.run(['xcrun','coremlcompiler','compile',str(package),str(target)],check=True,
                       env={**os.environ,'DEVELOPER_DIR':'/Applications/Xcode.app/Contents/Developer'})
        models.append(ct.models.MLModel(str(package),compute_units=ct.ComputeUnit.CPU_ONLY))
    vectors = []
    # Export golden outputs for the unchanged interface cases; no corpus evaluation on Mac.
    template = json.loads((BASE/'test_vectors.json').read_text())
    for original in template['vectors']:
        values = {n:np.asarray(v,np.int32) for n,v in original['inputs'].items()}
        memory = models[0].predict({'context_ids':values.pop('context_ids')})
        score = models[1].predict({**values,**memory})['logprob']
        assert score.shape==values['candidate_ids'].shape and np.isfinite(score).all()
        np.testing.assert_allclose(np.exp(score).sum(-1),1,atol=.01,rtol=.01)
        vectors.append({**original,'expected':{**{n:tensor(v) for n,v in memory.items()},'logprob':score.tolist()}})
    save(target/'test_vectors.json',dict(version=1,backend='Core ML CPU_ONLY',vectors=vectors,
         memory_format=template['memory_format'],
         comparison=dict(coreml_logprob_atol=.002,coreml_logprob_rtol=.002,pytorch_fp32_atol=.1),
         package_sha256={str(p.relative_to(target)):digest(p) for name in ['ContextEncoder','SyllableEncoder']
                         for p in (target/f'{name}.mlpackage').rglob('*') if p.is_file()}))
    if args.compatible:
        (target/'NOTICE.md').write_text(
            '# ZhuyinType compatible homophone continuation\n\n'
            'Model and derived corpus: CC BY-SA 4.0, https://creativecommons.org/licenses/by-sa/4.0/ . Code: AGPL-3.0-only.\n\n'
            'Chinese Wikipedia contributors: article paragraphs and talk-page revisions, CC BY-SA 4.0. '
            'Retain articles.tsv.gz and attribution.jsonl (revision/history links). '
            'https://foundation.wikimedia.org/wiki/Policy:Terms_of_Use#7._Licensing_of_Content\n\n'
            'Legislative Yuan, Taiwan, 2024–2025 gazettes: OGDL v1, https://data.gov.tw/license . '
            'Via OpenFun LYAPI, CC BY 4.0, https://v2.ly.govapi.tw/ . '
            'This open data is supplied by the Legislative Yuan under the Open Government Data License. '
            'Original document identities and URLs are retained in attribution.jsonl.\n\n'
            'Common Voice sentence-collector contributors: CC0 1.0, commit '
            '20fd76cd7d3b1e937fe99359b2ef3649b9c466ac. '
            'https://creativecommons.org/publicdomain/zero/1.0/\n\n'
            'Deduplicated existing colloquial corpus: Qwen3-8B-generated sentences and Common Voice. '
            'Qwen3-8B weights: Apache-2.0 (Qwen3-8B-LICENSE); generated text has no additional output-use restriction.\n\n'
            'Seed lexicon: CC-CEDICT CC BY-SA 4.0; Unihan Unicode License v3; wordfreq data CC BY-SA 4.0. '
            'Preserve component notices.\n\n'
            'Changes: continue original v3 with heldout 8-character/context exclusions, real-source supplement, '
            'balanced sentence-initial sampling and 3x confusion-position loss. '
            'See training/homophone/REPORT.md for validation and limitations. '
            'Retain this notice, source-manifest.json, attribution.jsonl, articles.tsv.gz and component notices. '
            'No source organization endorses this model.\n')
        for name in ['attribution.jsonl','source-manifest.json']:
            shutil.copy2(ROOT/'data/build/lm_chat2'/name,target/name)
        shutil.copy2(ROOT/'data/build/corpus/raw/Qwen3-8B-LICENSE',target/'Qwen3-8B-LICENSE')
        shutil.copytree(ROOT/'clients/macos/Resources/Licenses',target/'Licenses',dirs_exist_ok=True,
                        ignore=shutil.ignore_patterns('*Taiwan*','*Tongues*'))
        if (target/'stabilization.json').exists():
            with (target/'NOTICE.md').open('a') as notice:
                notice.write('\nStabilization: average the licensed continuation and original v3 decoder parameters equally; frozen context is identical. Parent hashes and coefficient are in stabilization.json.\n')
        return
    (target/'NOTICE.md').write_text(
        '# Homophone continuation\n\nSame v3 architecture and vocabularies.\n'
        'Wikipedia: CC BY-SA 4.0; retain articles.tsv.gz attribution. Lexicon: existing component terms.\n'
        'Taiwan Tongues zh-TW training TSV: adi-gov-tw and Common Voice contributors; TRAIL v0.1.\n'
        'Changes: holdout exclusions, confusion oversampling and weighted loss.\n'
        'Validation status: see training/homophone/REPORT.md and saved-output numerical audit.\n'
        'See copied source notices/licenses. Distribution compatibility has not been assessed.\n')
    for name in ['Taiwan-Tongues-LICENSE','Taiwan-Tongues-README.md','Qwen3-8B-LICENSE']:
        shutil.copy2(OUT/'lm_chat'/name,target/name)


if __name__=='__main__':
    main()
