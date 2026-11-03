"""Generate reproducible per-keystroke Go reference fixtures (Go oracle required)."""
import json, pathlib, random, re, subprocess
root = pathlib.Path(__file__).resolve().parents[2]
source = (root / 'upstream/bamboo-core/bamboo_test.go').read_text(encoding='utf-8')
words = set(re.findall(r'ProcessString\("([^"\n]*)"', source))
words.update(['ass','hoaf','uyr','toasn','gisa','gieengs','quawcj','nguwowfi','muoosi','khuya','loanj\b','chuyrene','turyn\b\b'])
rng = random.Random(421)
initials = ['','b','ch','gi','qu','ng','ngh','th','tr','kh','d','dd']
nuclei = ['a','aa','aw','e','ee','i','iee','o','oo','ow','u','uw','uo','uoo','uow','uwo','uwoi','uya','uyee','oa','uy','uyaw']
for _ in range(1800):
    word = rng.choice(initials)+rng.choice(nuclei)+rng.choice(['','n','ng','nh','c','ch','t','m','p'])
    tone = rng.choice(['s','f','r','x','j','z','ss','ww','oo'])
    pos = rng.randrange(len(word)+1)
    word = word[:pos]+tone+word[pos:]
    if rng.random()<.3: word+='\b'*rng.randrange(1,4)
    words.add(word)
requests=[]
for method in ['Telex','Telex 2','VNI','VIQR']:
    for flags in [7,5,0]:
        for word in sorted(words):
            requests.append(dict(Keys=word,Method=method,Flags=flags))
p=subprocess.run([str(root/'tests/tools/oracle.exe')],input='\n'.join(json.dumps(r) for r in requests)+'\n',text=True,encoding='utf-8',capture_output=True,check=True)
out=[]
for request,line in zip(requests,p.stdout.splitlines(),strict=True):
    request['states']=json.loads(line);out.append(request)
(root/'tests/go_reference.json').write_text(json.dumps(out,ensure_ascii=False,separators=(',',':')),encoding='utf-8')
print(f'{len(out)} cases, {sum(len(x["states"]) for x in out)} keystroke states')
