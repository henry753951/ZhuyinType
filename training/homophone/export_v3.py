"""Final Mac export: compile packages and emit matching portable golden vectors."""
import importlib
import json
import os
import shutil
import subprocess
import sys

from common import BASE, OUT, ROOT, digest, save, v3


def main():
    assert sys.platform=='darwin', 'Core ML export requires the Mac.'
    target = ROOT/'data/build/v3_homophone'
    assert 'v3_homophone' in json.loads((OUT/'choice.json').read_text())['models'], 'Export only the dev winner.'
    shutil.copy2(OUT/'lexicon.tsv',target/'lexicon.tsv')
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
        model.license = 'See NOTICE.md: CC BY-SA 4.0, component terms, and Taiwan Tongues TRAIL v0.1.'
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
