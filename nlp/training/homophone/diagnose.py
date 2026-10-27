"""Count primary errors introduced or rescued by the reranker on identical dev inputs."""
import argparse
from collections import Counter
import json
import sys

from common import OUT, jsonl, save
from eval import align


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--new',action='store_true')
    args=p.parse_args()
    assert sys.platform=='linux'
    plain,reranked=('v3-new','v2b-v3-new') if args.new else ('v3','v2b-v3')
    rows=list(jsonl(OUT/'dev_zai.jsonl'))
    report={}
    for ctx in [0,1]:
        predictions=[json.loads((OUT/'results'/f'{name}-dev_zai-ctx{ctx}.json').read_text())['predictions'] for name in [plain,reranked]]
        counts=Counter()
        for row,a,b in zip(rows,*predictions,strict=True):
            ma,mb=align(row['text'],a),align(row['text'],b)
            for pos in row['positions']:
                if pos['group']!='在再':
                    continue
                ia,ib=ma[pos['index']]==pos['char'],mb[pos['index']]==pos['char']
                counts['positions']+=1
                counts['reranked_errors']+=not ib
                counts['introduced']+=ia and not ib
                counts['rescued']+=ib and not ia
                counts['both_wrong']+=not ia and not ib
        assert counts['introduced']+counts['both_wrong']==counts['reranked_errors']
        report[str(ctx)]={**counts,'mostly_reranker':counts['introduced']>counts['reranked_errors']/2}
    save(OUT/('diagnosis-new.json' if args.new else 'diagnosis.json'),report)
    print(json.dumps(report,indent=2))


if __name__=='__main__':
    main()
