#!/usr/bin/env python3
"""Render the app-only About window in both themes on an isolated test display."""
import argparse, hashlib, importlib.util, subprocess, tempfile
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__)
p.add_argument('--binary',type=Path,required=True);p.add_argument('--xvfb',default='Xvfb')
a=p.parse_args()
spec=importlib.util.spec_from_file_location('display',Path(__file__).with_name('private-display.py'));display=importlib.util.module_from_spec(spec);spec.loader.exec_module(display)
with tempfile.TemporaryDirectory(prefix='dpdf-about-check-') as directory:
    folder=Path(directory)
    with display.display(folder,a.xvfb) as env:
        hashes=[]
        for theme in ['light','dark']:
            path=folder/(theme+'.ppm')
            command=[str(a.binary.resolve()),'--smoke-about',str(path)]
            if theme=='dark':command.append('dark')
            result=subprocess.run(command,env=env,capture_output=True,text=True,timeout=25)
            assert result.returncode==0,result.stderr
            images=[path,path.with_name(theme+'-details.ppm')]
            for image in images:
                assert image.read_bytes().startswith(b'P6\n') and image.stat().st_size>300*300*3
            assert images[0].read_bytes()!=images[1].read_bytes(),'About details did not scroll'
            hashes.append(hashlib.sha256(path.read_bytes()).digest())
        assert hashes[0]!=hashes[1],'Light and dark About renders are identical'
print('PASS: About light/dark and scroll-to-details snapshots; no external link opened')
