#!/usr/bin/env python3
"""Compare exact main/candidate CI runtimes on original generated fixtures."""
from pathlib import Path
import hashlib, importlib.util, json, os, subprocess, time
from PIL import Image, ImageChops

source=Path('/tmp/dpdf-engine-upgrade')
root=Path('/tmp/dpdf-engine-comparison');root.mkdir(exist_ok=True)
def module(name,file):
    spec=importlib.util.spec_from_file_location(name,source/'scripts'/file)
    m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m
sample=module('sample','make-sample.py');bench=module('bench','benchmark-reader.py')
runtimes={name:Path(path) for name,path in [('old','/tmp/dpdf-engine-baseline-main'),('new','/tmp/dpdf-engine-qualified-linux')]}
reports={name:json.loads((path/'report.json').read_text()) for name,path in runtimes.items()}
def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
for name,path in runtimes.items():
    assert digest(path/'runtime/dpdf')==reports[name]['binary_sha256']
    assert digest(path/'runtime/libpdfium.so')==reports[name]['pdfium_sha256']
def run(name,*args):
    p=runtimes[name];env={k:v for k,v in os.environ.items() if not k.startswith(('DPDF_','SLINT_'))}
    env.update(DPDF_PDFIUM_LIBRARY=str(p/'runtime/libpdfium.so'),DPDF_QPDF_BINARY='/usr/bin/qpdf',DPDF_CONFIG_DIR=str(root/name/'config'),DPDF_DATA_DIR=str(root/name/'data'))
    result=subprocess.run([str(p/'runtime/dpdf'),*map(str,args)],env=env,capture_output=True,timeout=45)
    if result.returncode:raise RuntimeError((name,args,result.returncode,result.stderr.decode(errors='replace')))
    return result.stdout
fixtures=[]
for rotation in (0,90,180,270):
 for crop in (False,True):
  for inherited in (False,True):
   for unit in (0.5,1,2):
    fixtures.append((f'geometry-r{rotation}-crop{int(crop)}-inherit{int(inherited)}-unit{unit}',bench.geometry_fixture(sample,rotation,crop,inherited,unit),1,'Reader geometry',1000))
for rotation in (0,90,180,270):
 for crop in (False,True):fixtures.append((f'contract-r{rotation}-crop{int(crop)}',sample.make_pdf(3,rotation,crop),2,'payment due',1000))
for unit in (0.5,1,2):fixtures.append((f'physical-unit{unit}',sample.make_physical_unit_pdf(unit),3,'Physical units',1000))
for page in (1,500,1000):fixtures.append((f'thousand-page{page}',sample.make_pdf(1000),page,'payment due',1000))
fixtures.append(('vector-detail',bench.vector_fixture(sample,1,1000),1,'Generated vector contract',2400))
report=dict(schema=1,status='IN_PROGRESS',scope='generated Linux old/new paired runtime comparison',old={k:reports['old'][k] for k in ('source_commit','binary_sha256','pdfium_sha256','run_url')},new={k:reports['new'][k] for k in ('source_commit','binary_sha256','pdfium_sha256','run_url')},cases=[],unqualified=['Fedora source build','Windows manual native acceptance','real documents and printers','malformed JPEG repair regression','security clearance'])
def save():(root/'report.json').write_text(json.dumps(report,indent=2)+'\n')
save()
try:
 for name,data,page,query,width in fixtures:
    folder=root/name;folder.mkdir(exist_ok=True);pdf=folder/'source.pdf';pdf.write_bytes(data);before=digest(pdf);case=dict(name=name,source_sha256=before,page=page,width=width)
    results={}
    for version in ('old','new'):
        frame=folder/(version+'.ppm');run(version,'--render-page',pdf,page,frame,width)
        text=json.loads(run(version,'--probe-select',pdf,page,0,0,1,1))
        search=json.loads(run(version,'--probe-search',pdf,query))
        probe=run(version,'--probe-page',pdf,page).decode().strip()
        # Time is deliberately excluded; retain actual diagnostics separately.
        (folder/(version+'-probe.txt')).write_text(probe+'\n')
        results[version]=dict(text=text,search=search,probe={k:v for k,v in (field.split('=',1) for field in probe.split()) if k not in ('open_ms','first_frame_ms')},render_sha256=digest(frame))
        (folder/(version+'-text-search.json')).write_text(json.dumps(results[version],indent=2)+'\n')
    a,b=(Image.open(folder/(v+'.ppm')).convert('RGB') for v in ('old','new'))
    case.update(dimensions_equal=a.size==b.size,dimensions=list(a.size),text_equal=results['old']['text']==results['new']['text'],search_equal=results['old']['search']==results['new']['search'],probe_equal=results['old']['probe']==results['new']['probe'],old=results['old'],new=results['new'])
    if a.size==b.size:
        diff=ImageChops.difference(a,b);hist=diff.histogram();case.update(changed_pixels=sum(pixel!=(0,0,0) for pixel in diff.getdata()),mean_abs_channel_error=sum((i%256)*n for i,n in enumerate(hist))/(a.width*a.height*3),max_channel_error=max((i%256 for i,n in enumerate(hist) if n),default=0))
    assert digest(pdf)==before,'source changed'
    case['source_unchanged']=True
    case['status']='IDENTICAL' if all(case[k] for k in ('dimensions_equal','text_equal','search_equal','probe_equal')) and case.get('changed_pixels')==0 else 'DIFFERENCE_REQUIRES_REVIEW'
    report['cases'].append(case);save();print(name,case['status'],case.get('changed_pixels'),flush=True)
 report['status']='PASS_IDENTICAL' if all(x['status']=='IDENTICAL' for x in report['cases']) else 'DIFFERENCES_REQUIRE_REVIEW'
except Exception as e:
 report['status']='FAIL';report['error']=str(e);raise
finally:save()
