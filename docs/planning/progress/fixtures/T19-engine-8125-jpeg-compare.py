from pathlib import Path
import importlib.util,json,hashlib,subprocess,os
from PIL import Image,ImageChops
source=Path('/tmp/dpdf-engine-upgrade');root=Path('/tmp/dpdf-engine-jpeg-comparison');root.mkdir(exist_ok=True)
s=importlib.util.spec_from_file_location('sample',source/'scripts/make-sample.py');sample=importlib.util.module_from_spec(s);s.loader.exec_module(sample)
original=(source/'scripts/fixtures/page-photo.jpg').read_bytes();assert original[-2:]==b'\xff\xd9'
# Locate the baseline DCT start-of-frame by parsing segment lengths, without
# interpreting entropy data or assuming a fixed metadata offset.
offset=2;height=None
while offset<len(original):
 assert original[offset]==255
 marker=original[offset+1]
 if marker==0xc0:height=offset+5;break
 assert marker not in (0xda,0xd9)
 length=int.from_bytes(original[offset+2:offset+4],'big');assert length>=2;offset+=2+length
assert height is not None and int.from_bytes(original[height:height+2],'big')==200
assert int.from_bytes(original[height+2:height+4],'big')==400
badheight=original[:height]+b'\xff\xff'+original[height+2:]
cases={'valid':original,'invalid-trailer':original[:-2]+b'\x00\x00','invalid-height':badheight,'both-repairs':badheight[:-2]+b'\x00\x00'}
runtimes={'old':Path('/tmp/dpdf-engine-baseline-main'),'new':Path('/tmp/dpdf-engine-qualified-linux')}
report=dict(schema=1,status='IN_PROGRESS',scope='generated malformed JPEG display regression, not a memory-safety exploit or internal COW proof',upstream_fix='4150fb564e0d83109f33a06385d22bb13cc3cd97',public_upstream_fix_contains_regression_fixture=False,jpeg_source_sha256=hashlib.sha256(original).hexdigest(),cases=[])
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def save():(root/'report.json').write_text(json.dumps(report,indent=2)+'\n')
save()
try:
 for name,jpeg in cases.items():
  folder=root/name;folder.mkdir(exist_ok=True);pdf=folder/'source.pdf'
  content=b'q 400 0 0 200 50 100 cm /I1 Do Q\nBT /F1 16 Tf 50 350 Td (JPEG repair fixture) Tj ET\n'
  pdf.write_bytes(sample.write_pdf([b'<< /Type /Catalog /Pages 2 0 R >>',b'<< /Type /Pages /Count 1 /Kids [3 0 R] >>',b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 500 400] /Resources << /XObject << /I1 5 0 R >> /Font << /F1 6 0 R >> >> /Contents 4 0 R >>',f'<< /Length {len(content)} >>\nstream\n'.encode()+content+b'endstream',f'<< /Type /XObject /Subtype /Image /Width 400 /Height 200 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode /Length {len(jpeg)} >>\nstream\n'.encode()+jpeg+b'\nendstream',b'<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>']))
  before=sha(pdf);result=dict(name=name,source_sha256=before,jpeg_sha256=hashlib.sha256(jpeg).hexdigest(),runtimes={})
  for version,p in runtimes.items():
   hosted=json.loads((p/'report.json').read_text());assert sha(p/'runtime/dpdf')==hosted['binary_sha256'];assert sha(p/'runtime/libpdfium.so')==hosted['pdfium_sha256']
   env={**os.environ,'DPDF_PDFIUM_LIBRARY':str(p/'runtime/libpdfium.so')}
   frame=folder/(version+'.ppm');subprocess.run([str(p/'runtime/dpdf'),'--render-page',str(pdf),'1',str(frame),'1000'],env=env,check=True,capture_output=True,timeout=30)
   image=Image.open(frame).convert('RGB');assert image.size==(1000,800)
   # The image bounds are x=100..900,y=200..600 at this raster size.
   assert sum(max(pixel)-min(pixel)>40 for pixel in image.crop((110,210,890,590)).get_flattened_data())>10000,'image failed to decode'
   baseline=root/'valid'/(version+'.ppm');valid=Image.open(baseline).convert('RGB');diff=ImageChops.difference(image,valid);changed=sum(p!=(0,0,0) for p in diff.get_flattened_data());assert changed==0,(name,version,changed)
   result['runtimes'][version]=dict(binary_sha256=hosted['binary_sha256'],pdfium_sha256=hosted['pdfium_sha256'],render_sha256=sha(frame),changed_pixels_vs_valid=changed)
  assert (folder/'old.ppm').read_bytes()==(folder/'new.ppm').read_bytes()
  assert sha(pdf)==before
  result.update(source_unchanged=True,old_new_pixels_identical=True,status='PASS');report['cases'].append(result);save();print(name,'PASS',flush=True)
 report['status']='PASS_GENERATED_DISPLAY_ONLY'
except Exception as e:
 report.update(status='FAIL',error=str(e));raise
finally:save()
