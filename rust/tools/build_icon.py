"""Rasterize our rectangle-only SVG icon using the Python standard library."""
from pathlib import Path
import struct,zlib,xml.etree.ElementTree as ET
root=Path(__file__).resolve().parents[1]
svg=ET.parse(root/'assets/claude-icon.svg').getroot()
rects=[]
def walk(el,fill='#000000'):
 fill=el.get('fill',fill)
 if el.tag.endswith('rect'):
  rects.append(tuple(float(el.get(k,'0')) for k in ('x','y','width','height'))+(tuple(bytes.fromhex(fill.lstrip('#')))+(255,),))
 for child in el:walk(child,fill)
walk(svg)
def png(n):
 pixels=bytearray(n*n*4)
 for y in range(n):
  for x in range(n):
   px=(x+.5)*64/n;py=(y+.5)*64/n
   for a,b,w,h,c in rects:
    if a<=px<a+w and b<=py<b+h:pixels[(y*n+x)*4:(y*n+x+1)*4]=bytes(c)
 def chunk(t,d):return struct.pack('>I',len(d))+t+d+struct.pack('>I',zlib.crc32(t+d)&0xffffffff)
 raw=b''.join(b'\0'+pixels[y*n*4:(y+1)*n*4] for y in range(n))
 return b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',n,n,8,6,0,0,0))+chunk(b'IDAT',zlib.compress(raw,9))+chunk(b'IEND',b'')
sizes=[16,24,32,48,64,128,256];images=[png(n) for n in sizes]
header=struct.pack('<HHH',0,1,len(sizes));offset=6+16*len(sizes);entries=b''
for n,data in zip(sizes,images):
 entries+=struct.pack('<BBBBHHII',n%256,n%256,0,0,1,32,len(data),offset);offset+=len(data)
(root/'assets/claude-icon.ico').write_bytes(header+entries+b''.join(images))
(root/'assets/claude-icon.png').write_bytes(images[-1])
print('Created ICO sizes',sizes)
