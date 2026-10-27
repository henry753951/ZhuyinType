"""Remote one-thread ONNX model-call timing on identical synthetic shapes."""
import argparse
import json
from pathlib import Path
import sys
import time

import numpy as np
import onnxruntime as ort

from common import ROOT, save


def measure(session, values):
    for _ in range(10):
        session.run(None,values)
    ms = []
    for _ in range(100):
        start = time.perf_counter()
        session.run(None,values)
        ms.append(1000*(time.perf_counter()-start))
    return dict(p50_ms=float(np.median(ms)),p95_ms=float(np.percentile(ms,95)),calls=len(ms))


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--v3',type=Path,required=True)
    p.add_argument('--reranker',type=Path,default=ROOT/'data/build/reranker')
    p.add_argument('--output',type=Path,required=True)
    args = p.parse_args()
    assert sys.platform=='linux'
    options = ort.SessionOptions()
    options.intra_op_num_threads=options.inter_op_num_threads=1
    context = ort.InferenceSession(str(args.v3/'ContextEncoder.onnx'),options,providers=['CPUExecutionProvider'])
    syllable = ort.InferenceSession(str(args.v3/'SyllableEncoder.onnx'),options,providers=['CPUExecutionProvider'])
    ctx = dict(context_ids=np.array([[1]+[4]*255],np.int32))
    memory = dict(zip(['memory_k','memory_v'],context.run(None,ctx)))
    values = {n:np.ones((1,30),np.int32)*v for n,v in [('syllable_ids',2),('tone_ids',1),('cost_bucket',0),('boundary',0)]}
    values['candidate_ids']=np.broadcast_to(np.arange(4,36,dtype=np.int32),(1,30,32)).copy()
    score=syllable.run(None,{**values,**memory})[0]
    assert score.shape==(1,30,32) and np.isfinite(score).all()
    reranker=ort.InferenceSession(str(args.reranker/'Reranker.onnx'),options,providers=['CPUExecutionProvider'])
    rr=dict(ids=np.ones((16,30),np.int32),targets=np.full((16,30),4,np.int32))
    score=reranker.run(None,rr)[0]
    assert score.shape==(16,30) and np.isfinite(score).all()
    save(args.output,dict(context=measure(context,ctx),syllable=measure(syllable,{**values,**memory}),
         reranker=measure(reranker,rr),
         protocol='Remote ONNX CPU, one thread, 10 warmups then 100 calls; v3 Lc=256 Ls=30 C=32; reranker B=16 L=30; bridge included; load excluded.'))


if __name__=='__main__':
    main()
