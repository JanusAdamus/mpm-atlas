from pathlib import Path
import pypdfium2 as pdfium
from PIL import Image,ImageOps,ImageDraw
B=Path(__file__).resolve().parent
out=B/'qa';out.mkdir(exist_ok=True)
p=pdfium.PdfDocument(str(B.parents[1]/'output/pdf/atlas-investigacion-y-plan.pdf'))
thumbs=[]
for i in range(len(p)):
    im=p[i].render(scale=1.35).to_pil().convert('RGB')
    im.save(out/f'page-{i+1:02}.png')
    thumb=ImageOps.contain(im,(238,337));tile=Image.new('RGB',(258,365),'#dce6e4');tile.paste(thumb,((258-thumb.width)//2,9));ImageDraw.Draw(tile).text((12,346),f'Página {i+1}',fill='black');thumbs.append(tile)
for start in range(0,len(thumbs),8):
    part=thumbs[start:start+8];sheet=Image.new('RGB',(258*4,365*2),'#c8d4d2')
    for j,t in enumerate(part):sheet.paste(t,((j%4)*258,(j//4)*365))
    sheet.save(out/f'contact-{start//8+1}.png')
print('Rendered',len(p),'pages')
