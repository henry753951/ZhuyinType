"""Reuse the v3 exporter and CPU validation without writing to the baseline tree."""
import importlib
import shutil

from common import BASE, OUT, ROOT, v3


def main():
    target = ROOT/'data/build/v3_homophone'
    for name in ['chars.json','syllables.json','lexicon.tsv','polyphone.pt','articles.tsv.gz']:
        shutil.copy2(BASE/name,target/name)
    v3()
    import common
    common.OUT = target
    export = importlib.import_module('export')
    export.export(target/'syllable.pt',target)
    import coremltools as ct
    for name in ['ContextEncoder','SyllableEncoder']:
        package = target/f'{name}.mlpackage'
        model = ct.models.MLModel(str(package), skip_model_load=True)
        model.license = 'See NOTICE.md: CC BY-SA 4.0, source component terms, and Taiwan Tongues TRAIL v0.1.'
        model.save(str(package))
    notice = ('# Homophone continuation\n\nFine-tuned ZhuyinType v3; same architecture and vocabularies.\n'
              'Wikipedia: CC BY-SA 4.0; retain articles.tsv.gz attribution. Lexicon: existing component terms.\n'
              'Taiwan Tongues zh-TW training TSV: adi-gov-tw and Common Voice contributors; TRAIL v0.1.\n'
              'Changes: source mining, exclusion of all specified holdouts, confusion oversampling and weighted loss.\n'
              'See copied source notices and licenses. Distribution compatibility has not been assessed.\n')
    (target/'NOTICE.md').write_text(notice)
    for name in ['Taiwan-Tongues-LICENSE','Taiwan-Tongues-README.md','Qwen3-8B-LICENSE']:
        shutil.copy2(OUT/'lm_chat'/name,target/name)


if __name__=='__main__':
    main()
