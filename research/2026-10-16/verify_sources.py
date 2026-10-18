import json, re, time, urllib.request, urllib.parse, subprocess, sys
from pathlib import Path
from html.parser import HTMLParser

BASE = Path(__file__).resolve().parent
RAW = BASE / 'sources'
RAW.mkdir(exist_ok=True)
refs = json.loads((BASE/'references.json').read_text(encoding='utf-8'))
ledger=[]

def fetch(url, name):
    req=urllib.request.Request(url,headers={'User-Agent':'MathematicsAtlasResearch/1.0 (public academic bibliography verification)'})
    try:
        with urllib.request.urlopen(req,timeout=35) as r:
            body=r.read(); status=r.status
        (RAW/name).write_bytes(body)
        ledger.append({'url':url,'file':name,'status':status,'bytes':len(body),'access_date':'2026-10-16'})
        return body
    except Exception as e:
        ledger.append({'url':url,'file':name,'error':str(e),'access_date':'2026-10-16'})
        return None

class Meta(HTMLParser):
    def __init__(self):
        super().__init__();self.meta={};self.text=[];self.ignore=0
    def handle_starttag(self,tag,attrs):
        a=dict(attrs)
        if tag in ('script','style'):self.ignore+=1
        if tag=='meta':
            key=a.get('name',a.get('property','')).lower()
            if key:self.meta.setdefault(key,[]).append(a.get('content',''))
    def handle_endtag(self,tag):
        if tag in ('script','style'):self.ignore=max(0,self.ignore-1)
    def handle_data(self,d):
        if not self.ignore and d.strip():self.text.append(d.strip())

arxivrefs=[r for r in refs if 'arxiv.org/abs/' in r['url']]
ids=[r['url'].rsplit('/',1)[1] for r in arxivrefs]
query='https://export.arxiv.org/api/query?'+urllib.parse.urlencode({'id_list':','.join(ids),'max_results':len(ids)})
body=(RAW/'arxiv-batch.xml').read_bytes() if (RAW/'arxiv-batch.xml').exists() else fetch(query,'arxiv-batch.xml')
if body:
    parser=Path('C:/Users/AdrianGonzalez/.agents/skills/paper-lookup/scripts/arxiv_atom.py')
    p=subprocess.run([sys.executable,'-X','utf8',str(parser),str(RAW/'arxiv-batch.xml')],capture_output=True,text=True,encoding='utf-8')
    (RAW/'arxiv-parsed.json').write_text(p.stdout,encoding='utf-8')
    if p.returncode:print('arXiv parser error:',p.stderr,flush=True)
    else:
        parsed=json.loads(p.stdout)
        entries=parsed.get('records',parsed.get('entries',parsed.get('results',[]))) if isinstance(parsed,dict) else parsed
        print('arXiv fields:',list(parsed)[:8] if isinstance(parsed,dict) else 'list',flush=True)
        for ref in arxivrefs:
            match=next((e for e in entries if e.get('arxiv_id')==ref['url'].rsplit('/',1)[1]),None)
            if match:
                ref['verified_metadata']=match;ref['review_level']='resumen y metadatos de arXiv';ref['metadata_source']='arxiv API'
                if match.get('authors'):ref['authors']=match['authors']

for ref in refs:
    if ref.get('doi'):
        url='https://api.crossref.org/works/'+urllib.parse.quote(ref['doi'],safe='')
        data=fetch(url,ref['id']+'-crossref.json')
        if data:
            obj=json.loads(data)
            if obj.get('status')=='ok' and obj.get('message-type')=='work':
                m=obj['message'];ref['crossref_title']=m.get('title',[]);ref['authors']=[' '.join([a.get('given',''),a.get('family','')]).strip() for a in m.get('author',[])]
                ref['venue']=m.get('container-title',[]);ref['crossref_year']=m.get('published',{}).get('date-parts',[[None]])[0][0]
                ref['crossref_updates']=m.get('update-to',[]);ref['crossref_relation']=m.get('relation',{})
                if ref['review_level'].startswith('pendiente'):ref['review_level']='metadatos editoriales Crossref; contenido contrastado por búsqueda web'
        time.sleep(.3)
    elif 'arxiv.org/abs/' not in ref['url']:
        data=fetch(ref['url'],ref['id']+'.html')
        if data:
            h=Meta();h.feed(data.decode('utf-8',errors='replace'))
            (RAW/(ref['id']+'.txt')).write_text('\n'.join(h.text),encoding='utf-8')
            ref['page_metadata']=h.meta
            if h.meta.get('citation_author'):ref['authors']=h.meta['citation_author']
            ref['review_level']='página primaria; alcance de lectura indicado en matriz'
    print(ref['id'],ref['review_level'],flush=True)

(BASE/'references.json').write_text(json.dumps(refs,ensure_ascii=False,indent=2),encoding='utf-8')
(BASE/'retrieval-ledger.json').write_text(json.dumps(ledger,ensure_ascii=False,indent=2),encoding='utf-8')
print('Saved',len(refs),'references;',len(ledger),'requests;',sum('error' in x for x in ledger),'errors',flush=True)
