"""Bounded, resumable multi-turn requests to an existing loopback Qwen3-8B server."""
import argparse
import asyncio
import importlib.util
import json
from pathlib import Path
import random
import sys
import time
import aiohttp

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('old_generator', ROOT / 'training/corpus/generate.py')
old = importlib.util.module_from_spec(spec)
spec.loader.exec_module(old)
OUT = ROOT / 'data/build/lm_chat2'
RAW = ROOT / 'data/raw/lm_chat2'
SYSTEM = ('你是台灣日常聊天語料撰寫者，只寫原創的自然繁體中文。不要引用新聞、書籍、真實貼文或歌詞。'
          '請寫台灣人用LINE和熟人傳訊息的多輪對話，前後要有回應，短句為主。用台不用臺。'
          '用台灣的詞彙、適量常見口語、俚語與啦喔欸耶齁，但不要每句都硬加語助詞。'
          '不要錯字、注音文、個人電話地址、簡體字、中國用語、說教或制式客服文案。'
          '每輪直接回應上一輪，人物關係和情境要一致，不要為了反轉而互相矛盾，不編造奇怪設施。'
          '只輸出JSON物件，格式為{"dialogues":[["A的話","B的話",...],...]}。每個陣列是一段獨立對話，交替說話。')


def prompt(index):
    rng = random.Random(20261027 + index)
    topic = rng.choice(old.TOPICS)
    persona = rng.sample(old.PERSONAS, 2)
    length = rng.randint(4, 12)
    situation = rng.choice(['臨時改變計畫', '互相確認小細節', '一人誤會另一人的意思', '討論普通小事',
                            '一人抱怨另一人安慰', '詢問建議再做決定', '回報事情的進展', '邀約但時間不合'])
    relation = rng.choice(['朋友', '家人', '熟同事', '同學', '室友', '伴侶'])
    return (f'只寫1段對話，主題是{topic}。人物是{persona[0]}和{persona[1]}，彼此是{relation}，'
            f'情境是{ situation }。每段{length}輪，每輪4到40字，長短自然交錯。有簡短回應、追問和補充，'
            f'每段有不同具體細節，不要固定開頭和結尾。不要出現人物標籤。創作種子{index}。')


async def main(args):
    RAW.mkdir(parents=True, exist_ok=True)
    OUT.mkdir(parents=True, exist_ok=True)
    path = RAW / 'generated.jsonl'
    done = set()
    if path.exists():
        with path.open('rb+') as f:
            offset = 0
            while line := f.readline():
                if not line.endswith(b'\n'):
                    f.truncate(offset)
                    break
                done.add(json.loads(line)['id'])
                offset = f.tell()
    queue = asyncio.Queue()
    for i in range(args.requests):
        if i not in done: queue.put_nowait(i)
    started = time.monotonic()
    failures = 0
    config = {'system': SYSTEM, 'seed': 20261027, 'model': 'Qwen3-8B', 'temperature': .95,
            'top_p': .96, 'presence_penalty': .2, 'max_tokens': 1400,
              'target_turns': 1000000, 'request_limit': args.requests, 'url': args.url,
              'revision': 'b968826d9c46dd6066d109eabc6255188de91218'}
    (OUT / 'generation-config.json').write_text(json.dumps(config, ensure_ascii=False, indent=2) + '\n')
    async with aiohttp.ClientSession(timeout=aiohttp.ClientTimeout(total=600)) as session:
        async with session.get(args.url + '/health') as response:
            response.raise_for_status()
        with path.open('a', buffering=1) as f:
            async def worker():
                nonlocal failures
                while not queue.empty() and failures < 3 and not (OUT / 'STOP_GENERATION').exists():
                    index = queue.get_nowait()
                    body = {k: config[k] for k in ('model', 'temperature', 'top_p', 'presence_penalty', 'max_tokens')}
                    body.update(seed=20261027 + index, chat_template_kwargs={'enable_thinking': False},
                                messages=[{'role': 'system', 'content': SYSTEM}, {'role': 'user', 'content': prompt(index)}])
                    try:
                        async with session.post(args.url + '/v1/chat/completions', json=body) as response:
                            response.raise_for_status()
                            result = await response.json()
                        f.write(json.dumps({'id': index, 'created': time.time(), 'prompt': body['messages'][1]['content'],
                                            'response': result}, ensure_ascii=False) + '\n')
                        done.add(index)
                        if len(done) % 25 == 0:
                            print(json.dumps({'requests': len(done), 'seconds': time.monotonic() - started}), flush=True)
                    except (aiohttp.ClientError, asyncio.TimeoutError, KeyError) as e:
                        failures += 1
                        print(f'FAILED id={index}: {e}', flush=True)
                if failures >= 3:
                    raise RuntimeError('three request failures; stopped, preserving raw responses')
            await asyncio.gather(*(worker() for _ in range(args.concurrency)))


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--url', default='http://127.0.0.1:8112')
    p.add_argument('--requests', type=int, default=8192)
    p.add_argument('--concurrency', type=int, default=64)
    asyncio.run(main(p.parse_args()))
