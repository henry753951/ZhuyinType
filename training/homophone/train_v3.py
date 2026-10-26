"""Continue the existing encoder, retain augmentation, select at dev plateau."""
from functools import lru_cache
import json
import random
import time
import argparse
import shutil
import sys

import numpy as np
import torch
from torch.nn import functional as F
from torch.utils.data import DataLoader, IterableDataset

from common import BASE, OUT, ROOT, jsonl, save, v3


class Batches(IterableDataset):
    def __init__(self, batch):
        self.batch = batch

    def __iter__(self):
        from dataset import Builder, Shard, collate
        worker = torch.utils.data.get_worker_info()
        rng = random.Random(812 + (worker.id if worker else 0))
        builder = Builder()
        manifest = json.loads((OUT/'filtered.json').read_text())['shards']
        groups = json.loads((OUT/'inventory.json').read_text())['kept']
        tt = list(jsonl(OUT/'tt-train.jsonl'))
        tt_by_char = {c:[r for r in tt if c in r['text']] for c in '在再'}
        load = lru_cache(maxsize=8)(Shard)
        @lru_cache(maxsize=512)
        def index(name, group):
            return np.load(OUT/'filtered'/f'{name}.{group}.npy', mmap_mode='r')
        while True:
            group = 'normal'
            if rng.random() < .3:
                group = 'primary-'+rng.choice('在再') if rng.random() < .5 else rng.choice(groups)['chars']
            weights = [(r['allowed'] if group=='normal' else r['groups'][group]) *
                       (2 if r['name'].startswith('colloquial') else 1) for r in manifest]
            examples = []
            for _ in range(self.batch):
                tt_pool = tt_by_char[group[-1]] if group.startswith('primary-') else []
                if tt_pool and rng.random()<.25:
                    row = rng.choice(tt_pool)
                    examples.append(builder.example(row['text'],[tuple(r) for r in row['readings']], '', rng))
                    continue
                row = rng.choices(manifest, weights)[0]
                shard = load(row['name'])
                choices = index(row['name'], group)
                start,end,doc = shard.examples[int(choices[rng.randrange(len(choices))])]
                text = ''.join(builder.vocab[n] if n>=4 else '\ufffd' for n in shard.ids[start:end])
                readings = [(builder.syllables[n],int(t)) for n,t in zip(shard.syllables[start:end],shard.tones[start:end])]
                context = ''.join(builder.vocab[n] if n>=4 else '\n' if n==3 else '\ufffd' if n==2 else '' for n in shard.ids[max(doc,start-255):start])
                examples.append(builder.example(text, readings, context, rng))
            yield collate(examples, builder, drop_context=rng.random()<.3)


def predict(context, model, builder, batch):
    from dataset import collate
    sid,tone,cost,boundary,candidates,ctx,target = [x.cuda() for x in collate(batch,builder)]
    with torch.autocast('cuda', dtype=torch.bfloat16):
        k,v = model.project(context.hidden(ctx), ctx!=0)
        scores = model(sid,tone,cost,boundary,candidates,k,v)
    return scores, candidates, target


def homophone_cases(builder):
    from readings import load_poly
    poly, labels = load_poly()
    cases = []
    for i,row in enumerate(jsonl(OUT/'dev_zai.jsonl')):
        readings,_ = builder.lex.label(row['text'],poly,builder.chars,labels)
        for enabled in [False,True]:
            e = builder.example(row['text'],readings,row['context'] if enabled else '', random.Random(i),rate=0,augment=False)
            e['focus'] = [p['index'] for p in row['positions'] if p['group']=='在再']
            cases.append(e)
    return cases


@torch.no_grad()
def score(context, model, builder, focused, general):
    model.eval()
    nll,total,correct = 0.,0,0
    for offset in range(0,len(focused),16):
        batch = focused[offset:offset+16]
        scores,candidates,target = predict(context,model,builder,batch)
        for i,e in enumerate(batch):
            for j in e['focus']:
                nll -= scores[i,j,target[i,j]].item()
                correct += scores[i,j].argmax().item() == target[i,j].item()
                total += 1
    result = dict(zai_nll=nll/total, zai_accuracy=correct/total, general={})
    for key,cases in general.items():
        hits,n = 0,0
        for offset in range(0,len(cases),16):
            batch = cases[offset:offset+16]
            scores,candidates,_ = predict(context,model,builder,batch)
            pred = candidates.gather(-1,scores.argmax(-1).unsqueeze(-1)).squeeze(-1).cpu().tolist()
            for e,p in zip(batch,pred):
                for i,(char,reading) in enumerate(zip(e['text'],e['readings'])):
                    if reading[0] != '<literal>':
                        hits += builder.vocab[p[i]] == char
                        n += 1
        result['general'][key] = dict(correct=hits, total=n, accuracy=hits/n)
    return result


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--steps',type=int,default=6000)
    p.add_argument('--batch',type=int,default=64)
    p.add_argument('--eval-every',type=int,default=500)
    args = p.parse_args()
    assert sys.platform == 'linux', 'Training and model evaluation run remotely.'
    assert not (OUT/'v3-training.json').exists(), 'Inspect existing training before explicitly resuming.'
    v3()
    from dataset import Builder
    from evaluate import cases
    from models import load_models
    torch.set_num_threads(4)
    torch.manual_seed(20261027)
    context,model = load_models(BASE/'syllable.pt','cuda')
    context.requires_grad_(False).eval()
    builder = Builder()
    focused = homophone_cases(builder)
    general = {f'{domain}-{ctx}-{noise}': list(cases(builder,domain,ctx,noise))
               for domain in ['colloquial','wikipedia'] for ctx in [False,True] for noise in [0.,.05]}
    baseline = score(context,model,builder,focused,general)
    save(OUT/'v3-baseline.json',baseline)
    print('baseline',json.dumps(baseline),flush=True)
    optimizer = torch.optim.AdamW(model.parameters(),lr=2e-5,weight_decay=.1,betas=(.9,.95),fused=True)
    stream = iter(DataLoader(Batches(args.batch),batch_size=None,num_workers=6,pin_memory=True,prefetch_factor=2))
    inventory = json.loads((OUT/'inventory.json').read_text())['kept']
    focus_ids = torch.tensor([builder.chars[c] for g in inventory for c in g['chars']],device='cuda')
    history,best,stale,best_step = [],baseline['zai_nll'],0,0
    started = time.monotonic()
    destination = ROOT/'data/build/v3_homophone'
    destination.mkdir(exist_ok=True)
    for name in ['context.pt','chars.json','syllables.json','config.json']:
        shutil.copy2(BASE/name,destination/name)
    for step in range(1,args.steps+1):
        model.train()
        for group in optimizer.param_groups:
            group['lr'] = 2e-5 * min(1.,step/100)
        sid,tone,cost,boundary,candidates,ctx,target = [x.cuda(non_blocking=True) for x in next(stream)]
        with torch.autocast('cuda',dtype=torch.bfloat16):
            with torch.no_grad():
                hidden = context.hidden(ctx)
            k,v = model.project(hidden,ctx!=0)
            scores = model(sid,tone,cost,boundary,candidates,k,v)
            loss = F.nll_loss(scores.flatten(0,1),target.flatten(),ignore_index=-100,reduction='none').view_as(target)
            truth = candidates.gather(-1,target.clamp_min(0).unsqueeze(-1)).squeeze(-1)
            weights = (1+2*torch.isin(truth,focus_ids)).float()*(target!=-100)
            loss = (loss*weights).sum()/weights.sum()
        if not torch.isfinite(loss):
            raise RuntimeError(f'nonfinite loss step {step}')
        optimizer.zero_grad(set_to_none=True)
        loss.backward()
        torch.nn.utils.clip_grad_norm_(model.parameters(),1.)
        optimizer.step()
        if step%100==0:
            print(step,loss.item(),flush=True)
        if step%args.eval_every==0:
            metrics = score(context,model,builder,focused,general)
            admissible = all(metrics['general'][k]['accuracy'] >= baseline['general'][k]['accuracy']-.002 for k in general)
            improved = admissible and metrics['zai_nll'] < best-.001
            if improved:
                best,stale,best_step = metrics['zai_nll'],0,step
                torch.save(dict(context=context.state_dict(),syllable=model.state_dict(),
                     vocab=len(builder.vocab),syllables=len(builder.syllables),step=step,metrics=metrics), destination/'syllable.pt')
            else:
                stale += 1
            row = dict(step=step,minutes=(time.monotonic()-started)/60,loss=loss.item(),admissible=admissible,metrics=metrics)
            history.append(row)
            print(json.dumps(row),flush=True)
            save(OUT/'v3-training.json',dict(history=history,selected_step=best_step,stale=stale,plateau=stale>=4,
                 batch=args.batch,oversample=.3,confusion_loss_weight=3,learning_rate=2e-5))
            if stale>=4:
                break
    if stale<4:
        raise RuntimeError('Step budget reached before dev plateau; do not export as complete.')
    if not best_step:
        raise RuntimeError('No checkpoint improved homophone dev under the general accuracy gate.')
    save(OUT/'v3-complete.json',dict(selected_step=best_step,plateau=True,seconds=time.monotonic()-started))


if __name__ == '__main__':
    main()
