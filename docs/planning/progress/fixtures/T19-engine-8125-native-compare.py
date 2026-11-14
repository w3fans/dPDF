from pathlib import Path
import importlib.util,subprocess,time,json,hashlib,os,shutil
from PIL import Image,ImageChops
root=Path('/tmp/dpdf-engine-native-comparison');root.mkdir(exist_ok=True)
source=Path('/tmp/dpdf-engine-upgrade')
def module(name,file):
 s=importlib.util.spec_from_file_location(name,source/'scripts'/file);m=importlib.util.module_from_spec(s);s.loader.exec_module(m);return m
private=module('private','private-display.py');sample=module('sample','make-sample.py')
runtimes={'old':Path('/tmp/dpdf-engine-baseline-main'),'new':Path('/tmp/dpdf-engine-qualified-linux')}
report=dict(schema=1,status='IN_PROGRESS',scope='actual X11 software windows; no physical/native compositor acceptance',cases=[])
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def save():(root/'report.json').write_text(json.dumps(report,indent=2)+'\n')
save()
try:
 for scale in (1,1.5,2):
  folder=root/('scale-'+str(scale));folder.mkdir(exist_ok=True);pdf=folder/'generated50.pdf';pdf.write_bytes(sample.make_pdf(50));before=sha(pdf)
  with private.display(folder,'Xvfb') as env:
   for name,runtime in runtimes.items():
    binary=runtime/'runtime/dpdf';hosted=json.loads((runtime/'report.json').read_text());assert sha(binary)==hosted['binary_sha256']
    config=folder/name;config.mkdir(exist_ok=True)
    childenv={**env,'DPDF_PDFIUM_LIBRARY':str(runtime/'runtime/libpdfium.so'),'DPDF_CONFIG_DIR':str(config/'config'),'DPDF_DATA_DIR':str(config/'data'),'DBUS_SESSION_BUS_ADDRESS':'unix:path='+str(config/'no-session-bus'),'SLINT_SCALE_FACTOR':str(scale)}
    with (folder/(name+'.log')).open('wb') as log:
     p=subprocess.Popen([str(binary),str(pdf)],env=childenv,stdout=log,stderr=log)
     try:
      deadline=time.monotonic()+15
      while time.monotonic()<deadline:
       r=subprocess.run(['xdotool','search','--all','--onlyvisible','--pid',str(p.pid)],env=childenv,capture_output=True,text=True)
       if r.returncode==0:break
       assert p.poll() is None;time.sleep(.1)
      else:raise RuntimeError('window absent')
      w=r.stdout.splitlines()[-1]
      def x(*args):subprocess.run(['xdotool',*map(str,args)],env=childenv,check=True,capture_output=True,timeout=10)
      x('windowsize',w,1180,800);x('windowfocus','--sync',w);time.sleep(1)
      x('mousemove','--window',w,900,500,'click',1);x('key','--clearmodifiers','--repeat',24,'--delay',40,'Next')
      # Wait for two complete consecutive images; a later semantic assessment
      # must additionally verify page 25 is actually visible.
      previous=None;deadline=time.monotonic()+10
      while time.monotonic()<deadline:
       time.sleep(.4);frame=folder/(name+'.png');subprocess.run(['import','-window',w,str(frame)],env=childenv,check=True,timeout=10)
       current=Image.open(frame).convert('RGB').tobytes()
       if current==previous:break
       previous=current
      else:raise RuntimeError('window did not settle')
     finally:
      if p.poll() is None:p.terminate()
      try:p.wait(timeout=5)
      except subprocess.TimeoutExpired:p.kill();p.wait()
    assert sha(pdf)==before
  a,b=(Image.open(folder/(n+'.png')).convert('RGB') for n in runtimes)
  assert a.size==b.size
  diff=ImageChops.difference(a,b);changed=sum(p!=(0,0,0) for p in diff.get_flattened_data())
  case=dict(scale=scale,source_sha256=before,source_unchanged=True,dimensions=list(a.size),old_screenshot_sha256=sha(folder/'old.png'),new_screenshot_sha256=sha(folder/'new.png'),changed_pixels=changed,status='IDENTICAL_VISUAL_ASSESSMENT_PENDING' if changed==0 else 'DIFFERENCE_REQUIRES_REVIEW')
  report['cases'].append(case);save();print(case,flush=True)
 report['status']='CAPTURED_VISUAL_ASSESSMENT_PENDING'
except Exception as e:
 report.update(status='FAIL',error=str(e));raise
finally:save()
