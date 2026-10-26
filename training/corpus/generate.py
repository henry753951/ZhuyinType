#!/usr/bin/env python3
"""Resumable, loopback-only vLLM requests; raw generations never overwrite a corpus."""
import argparse
import asyncio
import itertools
import json
import random
import time
from pathlib import Path
import aiohttp

TOPICS = '''早餐 午餐 晚餐 宵夜 便當 飲料 咖啡 早餐店 夜市 火鍋
買菜 煮飯 烘焙 洗碗 冰箱 外送 聚餐 素食 水果 食物過敏
上課 下課 作業 考試 報告 選課 社團 補習 畢業 宿舍
開會 加班 請假 通勤 面試 履歷 離職 薪水 同事 遠端工作
爸媽 手足 小孩 長輩 親戚 家務 搬家 租屋 鄰居 家庭聚會
約會 告白 失戀 相親 伴侶 生日 紀念日 道歉 吵架 安慰朋友
感冒 看診 掛號 牙醫 眼鏡 睡眠 運動 皮膚 頭痛 復健
下雨 颱風 天氣 預報 地震 空氣品質 寒流 悶熱 曬衣 除濕
捷運 公車 台鐵 高鐵 機車 汽車 計程車 停車 悠遊卡 腳踏車
網購 退貨 超商 超市 賣場 百貨公司 折扣 發票 集點 取貨
手遊 桌遊 電玩 遊戲更新 隊友 直播 排位賽 手把 電競 遊戲課金
手機 電腦 軟體 網路 印表機 密碼 充電 拍照 備份 藍牙
國內旅行 出國 行李 護照 機票 訂房 民宿 露營 爬山 海邊
貓 狗 寵物用品 獸醫 遛狗 餵食 浪貓 寵物洗澡 魚缸 鳥
開戶 轉帳 提款 信用卡 帳單 保險 繳稅 記帳 詐騙 存錢
戶政 郵局 區公所 健保 勞保 報稅 證件 里長 公共設施 垃圾分類
排球 籃球 棒球 羽球 游泳 跑步 健身 瑜伽 足球 單車
電影 影集 音樂 演唱會 動畫 漫畫 小說 展覽 舞台劇 廣播
洗衣 打掃 修水管 修冷氣 家具 收納 裝潢 電費 水費 瓦斯
剪頭髮 穿搭 化妝 婚禮 喪禮 廟會 志工 捐血 排隊 迷路'''.split()
REGISTERS = ['LINE聊天','當面口語對話','社群貼文','給同事的簡短信件','客服對話','生活日記','網路留言','問朋友的問題']
PERSONAS = ['大學生','上班族','帶小孩的家長','退休長輩','工程師','小店老闆','高中生','護理師','外送員','自由工作者']
LENGTHS = ['4到12字','12到24字','24到45字','45到70字']
SYSTEM = ('你是台灣在地日常語料撰寫者。只寫原創自然的台灣華語繁體中文，不引用歌曲、新聞、書籍或真實貼文。'
          '每行一則獨立短句或訊息，不編號、不加引號、不寫說明。用台灣用語如影片、軟體、網路、計程車、捷運、便當、機車、悠遊卡。'
          '避免中國大陸用語、簡體字、制式文案和雞湯。可自然使用喔、欸、啦、吧、耶、齁、超、真的假的、等一下、我跟你說，但不要每句都用。'
          '不必每句提到題目；內容要具體多變，有肯定、否定、疑問、抱怨、邀約、回應、提醒。少量英文可以。')

def prompt(index):
    rng = random.Random(index+20261026)
    topic = TOPICS[index%len(TOPICS)]
    register = REGISTERS[(index//len(TOPICS))%len(REGISTERS)]
    persona = PERSONAS[(index//(len(TOPICS)*len(REGISTERS)))%len(PERSONAS)]
    length = LENGTHS[(index//(len(TOPICS)*len(REGISTERS)*len(PERSONAS)))%len(LENGTHS)]
    detail = rng.choice(['臨時出了小狀況','很普通的日常小事','跟熟人互動','正在安排明後天','剛剛發生的事情','心情有點複雜','詢問或回覆一件小事','忍不住碎念一下'])
    return f'請寫32則互不重複的{register}，說話者是{persona}，主題是{topic}，情境是{detail}。每則約{length}。每則有不同內容與句型，不要一直用相同開頭，也不要寫標題。批次編號{index}（不要輸出編號）。'

async def main(args):
    root=Path(args.root); out=root/'data/build/corpus'; raw=out/'raw'; raw.mkdir(parents=True,exist_ok=True)
    (out/'generation-config.json').write_text(json.dumps({'model':'Qwen/Qwen3-8B','revision':'b968826d9c46dd6066d109eabc6255188de91218','topics':len(TOPICS),'registers':REGISTERS,'personas':PERSONAS,'lengths':LENGTHS,'temperature':.95,'top_p':.96,'presence_penalty':.45,'sentences_per_request':32,'concurrency':args.concurrency,'max_tokens':2600,'target_clean_sentences':args.target},ensure_ascii=False,indent=2)+'\n')
    path=raw/'synthetic.jsonl'
    done=set()
    if path.exists():
        with path.open('rb+') as f:
            offset=0
            for line in f:
                try: done.add(json.loads(line)['id'])
                except json.JSONDecodeError: f.truncate(offset); break
                offset=f.tell()
    queue=itertools.count(); lock=asyncio.Lock(); completed=len(done); started=time.time()
    timeout=aiohttp.ClientTimeout(total=1200)
    async with aiohttp.ClientSession(timeout=timeout,connector=aiohttp.TCPConnector(limit=args.concurrency,limit_per_host=args.concurrency)) as session:
        while True:
            try:
                async with session.get(args.url+'/health') as r:
                    if r.status==200: break
            except aiohttp.ClientError: pass
            await asyncio.sleep(10)
        with path.open('a',buffering=1) as output:
            async def worker():
                nonlocal completed
                while not (out/'STOP_GENERATION').exists():
                    progress=out/'clean-progress.json'
                    if progress.exists() and json.loads(progress.read_text()).get('accepted',0)>=args.target: return
                    index=next(queue)
                    if index in done: continue
                    body={'model':'Qwen3-8B','messages':[{'role':'system','content':SYSTEM},{'role':'user','content':prompt(index)}],
                          'temperature':.95,'top_p':.96,'presence_penalty':.45,'max_tokens':2600,
                          'seed':index+20261026,'chat_template_kwargs':{'enable_thinking':False}}
                    try:
                        async with session.post(args.url+'/v1/chat/completions',json=body) as response:
                            response.raise_for_status(); result=await response.json()
                        record={'id':index,'created':time.time(),'text':result['choices'][0]['message']['content'],
                                'finish_reason':result['choices'][0]['finish_reason'],'usage':result.get('usage',{})}
                        async with lock:
                            output.write(json.dumps(record,ensure_ascii=False)+'\n'); completed+=1
                            if completed%100==0: print(json.dumps({'completed_requests':completed,'seconds':time.time()-started}),flush=True)
                    except (aiohttp.ClientError, asyncio.TimeoutError,KeyError) as e:
                        print(f'request {index}: {e}',flush=True); await asyncio.sleep(5)
            await asyncio.gather(*(worker() for _ in range(args.concurrency)))

if __name__=='__main__':
    p=argparse.ArgumentParser(); p.add_argument('--root',default='.'); p.add_argument('--url',default='http://127.0.0.1:8102')
    p.add_argument('--concurrency',type=int,default=192); p.add_argument('--target',type=int,default=6000000)
    asyncio.run(main(p.parse_args()))
