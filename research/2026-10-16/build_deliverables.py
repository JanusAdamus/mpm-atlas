import json, re, html, csv, math
from pathlib import Path
from collections import Counter
from reportlab.platypus import SimpleDocTemplate, Paragraph, Spacer, Table, TableStyle, PageBreak, KeepTogether
from reportlab.lib.styles import getSampleStyleSheet, ParagraphStyle
from reportlab.lib import colors
from reportlab.lib.enums import TA_LEFT
from reportlab.pdfbase import pdfmetrics
from reportlab.pdfbase.ttfonts import TTFont
from reportlab.graphics.shapes import Drawing, Rect, String, Line, Polygon
from reportlab.graphics import renderSVG
from pypdf import PdfReader

B=Path(__file__).resolve().parent
OUT=B.parents[1]/'output'/'pdf'; OUT.mkdir(parents=True,exist_ok=True)
refs=json.loads((B/'references.json').read_text(encoding='utf-8'))
url_replacements={r['url']:r['url'].replace('(','%28').replace(')','%29') for r in refs}
for r in refs:r['url']=url_replacements[r['url']]
byid={r['id']:r for r in refs}
for rid in ('R04','R06','R22','R29'):
    byid[rid]['review_level']='resumen, metadatos y secciones seleccionadas del texto completo'
byid['R25']['review_level']='página oficial consultada con herramienta web; descarga directa fallida'
byid['R24'].update(title='Tangent-CFT: An Embedding Model for Mathematical Formulas',authors=['Behrooz Mansouri','Shaurya Rohatgi','Douglas W. Oard','Jian Wu','C. Lee Giles','Richard Zanibbi'],doi='10.1145/3341981.3344235',review_level='método y referencia contrastados en repositorio original; metadatos contrastados en DBLP')
byid['R44'].update(authors=['W. K. Adams','S. Reid','R. LeMaster','S. B. McKagan','K. K. Perkins','M. Dubson','C. E. Wieman'],review_level='referencia y principios contrastados en página de investigación de PhET; artículo completo no revisado')
docs={'R13','R25','R51','R52','R54','R58','R60'}
recent={'R04','R05','R06','R10','R11','R22','R48'}
for r in refs:
    r['source_type']='estándar o documentación primaria' if r['id'] in docs else ('preprint o informe técnico; estado editorial no confirmado' if r['id'] in recent else 'trabajo de investigación; versión consultada indicada por URL')
    r['interpretation']='La aplicación al atlas y el límite operativo son síntesis de diseño, no resultados ya demostrados en el atlas.'
    if r.get('verified_metadata'):
        r['arxiv_version']=r['verified_metadata'].get('version')
        r['arxiv_submission_date']=r['verified_metadata'].get('published')
(B/'references.json').write_text(json.dumps(refs,ensure_ascii=False,indent=2),encoding='utf-8')

raw=(B/'atlas-investigacion-y-plan.md').read_text(encoding='utf-8')
for old,new in url_replacements.items():raw=raw.replace(old,new)
raw=re.sub(r'\[(R\d{2})\](?!\()',lambda m:'['+m[1]+']('+byid[m[1]]['url']+')',raw)
(B/'atlas-investigacion-y-plan.md').write_text(raw,encoding='utf-8')

def shortauthors(r):
    a=r.get('authors',[])
    return ', '.join(a[:3])+(' et al.' if len(a)>3 else '') if a else 'Autoría institucional o no consignada'

bib=['# Bibliografía y matriz de evidencia','', 'Fecha de consulta: 16 de octubre de 2026. 60 referencias únicas. Aplicaciones y límites son síntesis para este proyecto; el nivel de lectura evita confundir un resumen con una revisión integral.','']
entries=[]
for r in refs:
    year=str(r['year']) if r['year'] else 'sin fecha'
    bib += [f"## {r['id']} · {r['title']}",'',f"{shortauthors(r)}. {year}. [{r['title']}]({r['url']}).",'',f"**Tipo:** {r['source_type']}. **Consulta:** {r['review_level']}.",'',f"**Técnica:** {r['technique']}",'',f"**Uso propuesto:** {r['application']}",'',f"**Límite:** {r['limitation']}",'']
    fields={'title':'{'+r['title']+'}','url':r['url'],'urldate':'2026-10-16','note':r['review_level']}
    if r.get('authors'):fields['author']=' and '.join(r['authors'])
    if r.get('year'):fields['year']=str(r['year'])
    if r.get('doi'):fields['doi']=r['doi']
    if 'arxiv.org/abs/' in r['url']:fields.update(eprint=r['url'].split('/')[-1],archivePrefix='arXiv')
    entries.append('@misc{'+r['id']+',\n'+',\n'.join('  '+k+' = {'+v.replace('&',r'\&')+'}' for k,v in fields.items())+'\n}')
(B/'bibliografia-y-evidencia.md').write_text('\n'.join(bib),encoding='utf-8')
(B/'references.bib').write_text('\n\n'.join(entries)+'\n',encoding='utf-8')

packages=[
('WP01','Modelo semántico y procedencia','Arquitectura + semántica',1,2,[],['R01','R02','R50','R51','R52'],'Esquema versionado; relaciones n-arias; supuestos; fuentes; historial; validaciones.','Todos los ejemplos de evaluación conservan contexto, evidencia y alternativas.'),
('WP02','Inventario y acuerdos de reutilización','Datos + edición',1,3,[],['R04','R07','R08','R53','R54'],'Registro por fuente de versión, identificadores, acceso, derechos y cobertura.','Cada conjunto incorporado tiene condiciones de uso y procedencia registradas.'),
('WP03','Corpus y benchmark de evaluación','Evaluación + especialistas',1,6,['WP01'],['R12','R20','R25','R33','R34'],'AtlasBench con 20,000 elementos anotados, particiones y adjudicación.','Separación temporal/documental; acuerdo entre revisores e incertidumbre reportados.'),
('WP04','Ingesta documental','Ingeniería de datos',2,5,['WP01','WP02'],['R08','R09','R10','R11','R13'],'Adaptadores LaTeX, HTML/XML y PDF; origen de cada bloque.','Pruebas por componente, fórmulas y referencias; reconstrucción de una importación.'),
('WP05','Extracción y resolución de entidades','NLP + semántica',2,8,['WP01','WP03','WP04'],['R14','R15','R16','R17','R18','R23'],'Candidatos tipados y fusiones reversibles con evidencia.','Precisión por clase; fusiones >99% y abstenciones declaradas en evaluación.'),
('WP06','Adaptadores formales','Matemáticas formales',2,10,['WP01','WP02'],['R03','R04','R06','R19','R20','R22'],'Importación versionada; separación de pruebas, enunciados e infraestructura.','Resultados reproducibles; fidelidad y prueba evaluadas de forma independiente.'),
('WP07','Recuperación híbrida','Búsqueda + ML',3,7,['WP03','WP04'],['R23','R24','R25','R26','R27','R28'],'Índices de texto, fórmulas y vectores; ranking y diagnóstico.','Evaluación contra baseline por idioma y dominio; p95 caliente <2s en carga declarada.'),
('WP08','Mapa multiescala','Visualización + UX',2,8,['WP01'],['R36','R37','R38','R39','R40'],'Geometría estable, capas, agregación y vistas accesibles.','Tareas de orientación evaluadas; interacción local p95 <100ms en hardware declarado.'),
('WP09','Síntesis con fuentes','NLP + edición',5,10,['WP05','WP07'],['R29','R30','R31','R32','R33','R35'],'Explicaciones y resúmenes regionales reconstruibles.','Respaldo y cobertura por afirmación; condiciones conservadas y errores auditados.'),
('WP10','Motor de recorridos','Semántica + producto',5,12,['WP05','WP07'],['R01','R15','R31','R43'],'Reglas de composición; rutas alternativas con supuestos y evidencia.','Validez de extremo a extremo >90% en conjunto independiente y abstenciones visibles.'),
('WP11','Aplicaciones interdisciplinarias','Edición + especialistas',4,18,['WP01','WP05'],['R16','R34','R45','R46','R47'],'Cadenas modelo-método-uso-evidencia y comparaciones.','Afirmaciones empíricas e influencias históricas con fuente propia.'),
('WP12','Aprendizaje y laboratorios','Aprendizaje + visualización',7,15,['WP08','WP10'],['R41','R42','R43','R44'],'Perfiles voluntarios, rutas educativas y 120 laboratorios.','Estudio de comprensión, retención y transferencia; simulaciones accesibles.'),
('WP13','Descubrimiento de puentes','ML + especialistas',10,18,['WP05','WP10','WP11'],['R45','R46','R47','R48','R56','R57'],'Candidatos de analogía y transferencia con contrastación.','Evaluación temporal y prospectiva; utilidad y carga de revisión publicadas.'),
('WP14','Contribución y federación','Plataforma + comunidad',4,18,['WP01','WP02'],['R49','R50','R51','R54','R55','R59'],'Edición revisable, APIs, exportaciones, réplicas y autoría semántica.','Edición reconstruible y exportación completa de datos publicables.'),
('WP15','Producción editorial bilingüe','Edición matemática',3,18,['WP01','WP05','WP14'],['R18','R40','R44','R59','R60'],'20,000 fichas, 100,000 conexiones revisadas y 1,000 recorridos como metas.','Doble control de casos críticos; conteos separados de automático, adaptado y original.'),
('WP16','Operación, calidad y reproducibilidad','Plataforma + evaluación',1,18,[],['R12','R25','R33','R35','R48'],'CI de datos; costos; auditorías; backups; pruebas de reconstrucción.','Métricas por edición; correcciones propagadas; benchmark y costos publicados.')]
plans=[dict(zip(['id','title','owner_role','start_month','end_month','dependencies','references','deliverable','acceptance'],p)) for p in packages]
(B/'plan-ejecucion.json').write_text(json.dumps(plans,ensure_ascii=False,indent=2),encoding='utf-8')
pm=['# Plan de ejecución','', 'Horizonte: 18 meses desde el inicio efectivo. Objetivos de ingeniería y calidad propuestos; la asignación de recursos se ajustará con mediciones. Las dependencias designan contratos o resultados requeridos, no impiden trabajo preparatorio simultáneo.','']
for p in plans:
    pm += [f"## {p['id']} · {p['title']}",'',f"**Responsable:** {p['owner_role']}. **Ventana:** meses {p['start_month']}-{p['end_month']}.",'',f"**Dependencias:** {', '.join(p['dependencies']) or 'ninguna'}. **Fuentes:** {', '.join(p['references'])}.",'',f"**Entregable:** {p['deliverable']}",'',f"**Aceptación:** {p['acceptance']}",'']
(B/'plan-ejecucion.md').write_text('\n'.join(pm),encoding='utf-8')

INK=colors.HexColor('#142d3c'); TEAL=colors.HexColor('#087f7b'); PALE=colors.HexColor('#e9f3f1'); GRAY=colors.HexColor('#4e6170'); GOLD=colors.HexColor('#c1934c')
for name,file in [('Atlas','segoeui.ttf'),('AtlasBold','segoeuib.ttf'),('AtlasItalic','segoeuii.ttf')]:pdfmetrics.registerFont(TTFont(name,str(Path('C:/Windows/Fonts')/file)))
pdfmetrics.registerFontFamily('Atlas',normal='Atlas',bold='AtlasBold',italic='AtlasItalic',boldItalic='AtlasBold')

def architecture():
    d=Drawing(490,285)
    levels=[('FUENTES','Bibliotecas formales · artículos · libros · aplicaciones'),('AFIRMACIONES CON CONTEXTO','Participantes · supuestos · evidencia · versión · revisión'),('ÍNDICES Y PROYECCIONES','Texto y fórmulas · vectores · grafos · regiones y niveles'),('SERVICIOS DE COMPRENSIÓN','Búsqueda · recorridos · explicaciones · experimentos'),('EXPERIENCIA ABIERTA','Mapa · lectura · cuadernos · autoría · API · exportación')]
    for i,(title,desc) in enumerate(levels):
        y=235-i*51
        d.add(Rect(6,y,478,40,rx=7,ry=7,fillColor=INK if i==1 else PALE,strokeColor=None))
        d.add(String(18,y+24,title,fontName='AtlasBold',fontSize=9,fillColor=colors.white if i==1 else TEAL))
        d.add(String(18,y+9,desc,fontName='Atlas',fontSize=8.2,fillColor=colors.white if i==1 else INK))
        if i<4:
            d.add(Line(245,y,245,y-10,strokeColor=TEAL,strokeWidth=1.2))
            d.add(Polygon([242,y-7,245,y-11,248,y-7],fillColor=TEAL,strokeColor=None))
    d.add(String(6,7,'La procedencia, las versiones y la evaluación atraviesan todas las capas.',fontName='AtlasItalic',fontSize=8.4,fillColor=GRAY))
    return d

def roadmap():
    d=Drawing(490,238);x0=151;step=18
    for m in range(1,19):d.add(String(x0+(m-1)*step+4,222,str(m),fontName='Atlas',fontSize=7.5,fillColor=GRAY))
    rows=[('Arquitectura y evaluación',1,18),('Ingesta y extracción',2,8),('Búsqueda y mapa',2,8),('Explicaciones y aplicaciones',4,18),('Recorridos y aprendizaje',5,15),('Descubrimiento',10,18),('Edición y federación',3,18)]
    for i,(label,s,e) in enumerate(rows):
        y=192-i*26;d.add(String(2,y+5,label,fontName='Atlas',fontSize=8.2,fillColor=INK))
        d.add(Rect(x0,y,18*step,16,fillColor=PALE,strokeColor=None))
        d.add(Rect(x0+(s-1)*step,y,(e-s+1)*step-2,16,rx=3,ry=3,fillColor=TEAL if i%2==0 else INK,strokeColor=None))
    d.add(String(2,7,'Meses relativos al inicio. Líneas de trabajo superpuestas; metas sujetas a evaluación.',fontName='AtlasItalic',fontSize=8,fillColor=GRAY))
    return d

for name,d in [('arquitectura',architecture()),('programa',roadmap())]:
    renderSVG.drawToFile(d,str(B/(name+'.svg')))

def inline(s,mode='pdf'):
    s=html.escape(s.replace('–','-').replace('—','-').replace('‑','-'))
    s=re.sub(r'\*\*(.+?)\*\*',r'<b>\1</b>',s)
    if mode=='pdf':
        s=re.sub(r'\[([^]]+)\]\((https?://[^)]+)\)',r'<link href="\2" color="#087f7b">\1</link>',s)
    else:
        s=re.sub(r'\[([^]]+)\]\((https?://[^)]+)\)',r'<a href="\2" target="_blank" rel="noopener">\1</a>',s)
    return s

def blocks(text):
    lines=text.splitlines();i=0
    while i<len(lines):
        l=lines[i].strip()
        if not l:i+=1;continue
        if l.startswith('#'):
            n=len(l)-len(l.lstrip('#'));yield('heading',n,l[n:].strip());i+=1
        elif l.startswith('|'):
            rows=[]
            while i<len(lines) and lines[i].strip().startswith('|'):
                row=[x.strip() for x in lines[i].strip().strip('|').split('|')]
                if not all(re.fullmatch(r'[:\- ]+',x) for x in row):rows.append(row)
                i+=1
            yield('table',rows)
        elif re.match(r'^\d+\. ',l):
            yield('item',l);i+=1
        else:
            p=[l];i+=1
            while i<len(lines) and lines[i].strip() and not lines[i].startswith(('#','|')) and not re.match(r'^\d+\. ',lines[i]):p.append(lines[i].strip());i+=1
            yield('paragraph',' '.join(p))

styles=getSampleStyleSheet()
styles.add(ParagraphStyle(name='BodyAtlas',fontName='Atlas',fontSize=10,leading=14.4,textColor=INK,spaceAfter=8))
styles.add(ParagraphStyle(name='HAtlas',fontName='AtlasBold',fontSize=15,leading=19,textColor=TEAL,spaceBefore=17,spaceAfter=9,keepWithNext=True))
styles.add(ParagraphStyle(name='SmallAtlas',fontName='Atlas',fontSize=8,leading=11,textColor=INK,spaceAfter=4))
styles.add(ParagraphStyle(name='RefAtlas',fontName='Atlas',fontSize=8.6,leading=12.3,textColor=INK,spaceAfter=10))
styles.add(ParagraphStyle(name='CaptionAtlas',fontName='AtlasItalic',fontSize=8.5,leading=12,textColor=GRAY,spaceAfter=14))

def footer(c,doc):
    c.setStrokeColor(PALE);c.line(48,42,547,42);c.setFont('Atlas',8);c.setFillColor(GRAY)
    c.drawString(48,29,'ATLAS / INVESTIGACIÓN Y PLAN / 16 OCT 2026');c.drawRightString(547,29,str(doc.page))

story=[]
story.append(Spacer(1,55))
story.append(Paragraph('PROYECTO ABIERTO · INVESTIGACIÓN TÉCNICA',ParagraphStyle('k',fontName='AtlasBold',fontSize=10,textColor=TEAL,leading=15)))
story.append(Spacer(1,24))
story.append(Paragraph('Atlas de las<br/>conexiones<br/>matemáticas',ParagraphStyle('cover',fontName='AtlasBold',fontSize=36,leading=42,textColor=INK)))
story.append(Spacer(1,22))
story.append(Paragraph('De la literatura a una infraestructura navegable, entendible y usable.',ParagraphStyle('sub',fontName='Atlas',fontSize=15,leading=22,textColor=GRAY)))
story.append(Spacer(1,32))
stats=Table([[Paragraph('<b>60</b><br/>referencias y recursos',styles['BodyAtlas']),Paragraph('<b>16</b><br/>paquetes de trabajo',styles['BodyAtlas']),Paragraph('<b>18 meses</b><br/>horizonte propuesto',styles['BodyAtlas'])]],colWidths=[163,163,163])
stats.setStyle(TableStyle([('BACKGROUND',(0,0),(-1,-1),PALE),('BOX',(0,0),(-1,-1),0,PALE),('LEFTPADDING',(0,0),(-1,-1),13),('TOPPADDING',(0,0),(-1,-1),12),('BOTTOMPADDING',(0,0),(-1,-1),9)]));story.append(stats)
story.append(Spacer(1,28));story.append(Paragraph('16 de octubre de 2026<br/>Síntesis crítica, arquitectura propuesta, evaluación y programa de ejecución.',styles['BodyAtlas']))
story.append(Spacer(1,10));story.append(Paragraph('Revisión de alcance orientada al diseño. Los objetivos del programa son propuestas; no son resultados ya obtenidos.',styles['CaptionAtlas']));story.append(PageBreak())
story.append(Paragraph('Dos esquemas para leer el proyecto',styles['HAtlas']));story.append(architecture());story.append(Paragraph('Arquitectura propuesta. El registro de afirmaciones conserva contexto y evidencia; los índices y vistas se reconstruyen desde él.',styles['CaptionAtlas']));story.append(roadmap());story.append(Paragraph('Programa propuesto. Las líneas se superponen y convergen en una experiencia de alcance completo.',styles['CaptionAtlas']));story.append(PageBreak())
for block in blocks(raw):
    if block[0]=='heading':
        if block[1]==1:continue
        story.append(Paragraph(inline(block[2]),styles['HAtlas']))
    elif block[0] in ('paragraph','item'):story.append(Paragraph(inline(block[1]),styles['BodyAtlas']))
    elif block[0]=='table':
        rows=block[1];n=len(rows[0]);widths=[112,177,201] if n==3 else [150,340]
        t=Table([[Paragraph(inline(c),styles['SmallAtlas']) for c in row] for row in rows],colWidths=widths,repeatRows=1,hAlign='LEFT')
        t.setStyle(TableStyle([('BACKGROUND',(0,0),(-1,0),PALE),('VALIGN',(0,0),(-1,-1),'TOP'),('LINEBELOW',(0,0),(-1,0),.8,TEAL),('LINEBELOW',(0,1),(-1,-1),.3,colors.HexColor('#d5dfdf')),('TOPPADDING',(0,0),(-1,-1),7),('BOTTOMPADDING',(0,0),(-1,-1),7),('LEFTPADDING',(0,0),(-1,-1),7),('RIGHTPADDING',(0,0),(-1,-1),7)]));story.append(t);story.append(Spacer(1,12))
story.append(PageBreak());story.append(Paragraph('Bibliografía seleccionada',styles['HAtlas']))
story.append(Paragraph('Los identificadores R01-R60 enlazan las decisiones del informe con las fuentes. La matriz separada contiene el nivel de lectura, técnica, uso propuesto y limitación de cada referencia.',styles['BodyAtlas']))
for r in refs:
    label=f"<b>{r['id']}</b> · {html.escape(shortauthors(r))}. {r['year'] or 's. f.'}. <link href=\"{html.escape(r['url'],quote=True)}\" color=\"#087f7b\">{html.escape(r['title'])}</link>."
    label+=' <font color="#4e6170">'+html.escape(r['source_type'])+'.</font>'
    story.append(Paragraph(label,styles['RefAtlas']))
pdfpath=OUT/'atlas-investigacion-y-plan.pdf'
doc=SimpleDocTemplate(str(pdfpath),pagesize=(595.28,841.89),rightMargin=51,leftMargin=51,topMargin=48,bottomMargin=56,title='Atlas de las conexiones matemáticas: investigación y plan',author='Proyecto Atlas')
doc.build(story,onFirstPage=footer,onLaterPages=footer)

body=[];toc=[];h=0
for block in blocks(raw):
    if block[0]=='heading':
        if block[1]==1:continue
        h+=1;sid='section-'+str(h);toc.append(f'<a href="#{sid}">{html.escape(block[2])}</a>');body.append(f'<h2 id="{sid}">{html.escape(block[2])}</h2>')
    elif block[0] in ('paragraph','item'):body.append('<p>'+inline(block[1],'html')+'</p>')
    elif block[0]=='table':
        rows=block[1];body.append('<div class="tablewrap"><table>')
        for i,row in enumerate(rows):
            tag='th' if i==0 else 'td';body.append('<tr>'+''.join(f'<{tag}>'+inline(c,'html')+f'</{tag}>' for c in row)+'</tr>')
        body.append('</table></div>')
cards=[]
for r in refs:
    search=html.escape(' '.join([r['id'],r['title'],shortauthors(r),r['theme'],r['technique'],r['application']]).lower(),quote=True)
    cards.append(f'''<article class="ref" data-theme="{r['theme']}" data-search="{search}"><div class="eyebrow">{r['id']} · {r['theme']} · {r['year'] or 's. f.'}</div><h3><a href="{html.escape(r['url'],quote=True)}" target="_blank" rel="noopener">{html.escape(r['title'])}</a></h3><p class="muted">{html.escape(shortauthors(r))}</p><p><b>Técnica.</b> {html.escape(r['technique'])}</p><p><b>Uso propuesto.</b> {html.escape(r['application'])}</p><p><b>Límite.</b> {html.escape(r['limitation'])}</p><details><summary>Tipo y alcance de revisión</summary><p>{html.escape(r['source_type'])}. {html.escape(r['review_level'])}.</p></details></article>''')
svg1=(B/'arquitectura.svg').read_text(encoding='utf-8');svg1=svg1[svg1.index('<svg'):]
svg2=(B/'programa.svg').read_text(encoding='utf-8');svg2=svg2[svg2.index('<svg'):]
css='''*{box-sizing:border-box}html{scroll-behavior:smooth;scroll-padding-top:24px}body{margin:0;background:#f6f8f7;color:#142d3c;font:16px/1.65 "Segoe UI",sans-serif}a{color:#087f7b;text-decoration-thickness:1px;text-underline-offset:3px}header{background:#142d3c;color:#fff;padding:64px max(5vw,28px)}header h1{font-size:clamp(34px,5vw,68px);line-height:1.08;letter-spacing:-2px;margin:20px 0;max-width:900px}.eyebrow{font-size:12px;text-transform:uppercase;letter-spacing:1.3px;color:#087f7b;font-weight:700}header .eyebrow{color:#a8dfd3}header p{max-width:760px;color:#d8e5e8}.stats{display:flex;gap:36px;margin-top:32px}.stats b{display:block;font-size:29px}.stats span{font-size:13px;color:#b4c9ce}.layout{max-width:1450px;margin:auto;display:grid;grid-template-columns:250px 1fr;gap:50px;padding:40px 32px}aside{position:sticky;top:24px;align-self:start;max-height:92vh;overflow:auto;font-size:13px}aside a{display:block;text-decoration:none;padding:6px 0;color:#405c69}aside strong{display:block;margin:14px 0}main{min-width:0;max-width:1000px}h2{font-size:27px;line-height:1.3;border-top:1px solid #d6e2df;padding-top:35px;margin-top:42px}h3{font-size:18px;line-height:1.4;margin:12px 0}.tablewrap{overflow:auto;margin:22px 0}table{border-collapse:collapse;width:100%;font-size:14px}th,td{text-align:left;vertical-align:top;padding:12px;border-bottom:1px solid #d6e2df}th{background:#e9f3f1}td:first-child{font-weight:600;min-width:150px}.diagrams{background:white;padding:20px;border-radius:12px;margin:24px 0}.diagrams svg{width:100%;height:auto;display:block;margin:20px auto;max-width:700px}.filters{display:flex;gap:12px;flex-wrap:wrap;position:sticky;top:0;background:#f6f8f7;padding:14px 0;z-index:2}input,select{font:inherit;padding:11px;border:1px solid #c6d5d0;border-radius:6px;background:white;min-width:180px}input{flex:1}.ref{background:#fff;border:1px solid #d6e2df;border-radius:10px;padding:23px;margin:15px 0}.ref p{margin:8px 0;font-size:14px}.muted,details{color:#536c76;font-size:13px}summary{cursor:pointer}.ref[hidden]{display:none}.links a{display:inline-block;margin:8px 20px 8px 0}footer{padding:30px 5vw;border-top:1px solid #d6e2df;color:#536c76;font-size:13px}a:focus-visible,input:focus-visible,select:focus-visible,summary:focus-visible{outline:3px solid #c1934c;outline-offset:3px}@media(max-width:950px){.layout{display:block;padding:24px}aside{position:static;max-height:none;display:none}.stats{gap:22px}header{padding:40px 24px}}@media print{aside,.filters{display:none}.layout{display:block}header{background:white;color:#142d3c}header p{color:#142d3c}}'''
options='<option value="">Todas las líneas de investigación</option>'+''.join(f'<option>{x}</option>' for x in sorted({r['theme'] for r in refs}))
htmltext='''<!doctype html><html lang="es"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Atlas · Investigación y plan</title><style>'''+css+'''</style></head><body><header><div class="eyebrow">Dossier de investigación · 16 octubre 2026</div><h1>Atlas de las conexiones matemáticas</h1><p>Una base de literatura para construir una infraestructura navegable, entendible y usable. Arquitectura, investigación propia y programa de alcance completo.</p><div class="stats"><div><b>60</b><span>referencias y recursos</span></div><div><b>16</b><span>paquetes de trabajo</span></div><div><b>18 meses</b><span>horizonte propuesto</span></div></div></header><div class="layout"><aside><strong>Leer el informe</strong>'''+''.join(toc)+'''<strong>Explorar las fuentes</strong><a href="#evidence">Bibliografía y matriz filtrable</a><a href="plan-ejecucion.md">Paquetes de trabajo</a><a href="references.bib">Bibliografía BibTeX</a></aside><main><section class="diagrams"><div class="eyebrow">Dos vistas del proyecto propuesto</div>'''+svg1+svg2+'''</section>'''+''.join(body)+'''<h2 id="evidence">Bibliografía y matriz de evidencia</h2><p>La búsqueda filtra por título, autor, tema, técnica y aplicación. Las recomendaciones son síntesis de diseño; el detalle de cada ficha declara qué se consultó.</p><div class="filters"><input id="q" aria-label="Buscar referencias" placeholder="Buscar: fórmulas, rutas, Lean, visualización…"><select id="theme" aria-label="Filtrar por tema">'''+options+'''</select></div><p id="count" aria-live="polite">60 referencias</p>'''+''.join(cards)+'''</main></div><footer>Revisión de alcance orientada al diseño. El atlas descrito es una propuesta; este archivo es el informe navegable. Las fuentes originales se abren en su sitio y este documento no requiere servicios externos para funcionar.</footer><script>const q=document.getElementById('q'),t=document.getElementById('theme'),cards=[...document.querySelectorAll('.ref')];function norm(s){return s.normalize('NFD').replace(/[\u0300-\u036f]/g,'').toLowerCase()}function filter(){const terms=norm(q.value).split(/\s+/).filter(Boolean);let n=0;cards.forEach(c=>{const ok=(!t.value||c.dataset.theme===t.value)&&terms.every(x=>norm(c.dataset.search).includes(x));c.hidden=!ok;if(ok)n++});document.getElementById('count').textContent=n+' de 60 referencias'}q.addEventListener('input',filter);t.addEventListener('change',filter);</script></body></html>'''
(B/'atlas-investigacion.html').write_text(htmltext,encoding='utf-8')

reader=PdfReader(str(pdfpath));text='\n'.join(p.extract_text() or '' for p in reader.pages)
missing=[r['id'] for r in refs if r['id'] not in text]
assert not missing,missing
assert len({r['url'] for r in refs})==60
assert len({r['id'] for r in refs})==60
for p in plans:
    assert set(p['references'])<=set(byid)
    assert set(p['dependencies'])<={x['id'] for x in plans}
summary={'references':len(refs),'pdf_pages':len(reader.pages),'work_packages':len(plans),'pdf_path':str(pdfpath),'html_path':str(B/'atlas-investigacion.html'),'source_types':dict(Counter(r['source_type'] for r in refs)),'checks':['60 unique IDs and source URLs','all references present in PDF','all work package dependencies and reference IDs resolve']}
(B/'delivery-checks.json').write_text(json.dumps(summary,ensure_ascii=False,indent=2),encoding='utf-8')
print(json.dumps(summary,ensure_ascii=False,indent=2))
