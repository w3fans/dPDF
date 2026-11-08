#!/usr/bin/env python3
"""Generated-only 32-case block-coordinate investigation; never qualifies production baking."""
import argparse
from datetime import datetime, timezone
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module

def main():
    scripts = Path(__file__).resolve().parent
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=Path('target/release/dpdf'))
    parser.add_argument('--pdfium', type=Path, required=True)
    parser.add_argument('--qpdf', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if not __debug__:
        parser.error('Run diagnostics without Python optimization so validation checks remain enabled')
    prototype = load('prototype_geometry', scripts / 'probe-combined-block.py')
    fixtures = load('fixtures_geometry', scripts / 'check-pages.py')
    engine = prototype.Engine(args.pdfium.resolve())
    binary = str(args.binary.resolve())
    qpdf = str(args.qpdf.resolve())
    env = dict(os.environ, DPDF_PDFIUM_LIBRARY=str(args.pdfium.resolve()), DPDF_QPDF_BINARY=qpdf)
    region = [90,60,280,140]


    def run(args):
        return subprocess.run(list(map(str,args)), env=env, check=True, capture_output=True, timeout=60).stdout


    def document(negative, cropped, inherited, rotation):
        media = '[-30 -20 370 280]' if negative else '[0 0 400 300]'
        crop = '[-10 0 350 260]' if negative else '[20 20 380 280]'
        box = f'/MediaBox {media} /Rotate {rotation} '.encode() + (f'/CropBox {crop} '.encode() if cropped else b'')
        content = b'BT /F1 16 Tf 30 240 Td (OUTSIDE searchable text) Tj ET\nBT /F1 10 Tf 100 105 Td (BAKE_LABEL Authorized person:) Tj ET\n100 90 m 270 90 l S\nBT /F1 10 Tf 100 70 Td (BAKE_DATE 2026) Tj ET\n'
        other = b'BT /F1 16 Tf 30 240 Td (UNTOUCHED second page) Tj ET\n'
        def stream(data):
            return f'<< /Length {len(data)} >>\nstream\n'.encode()+data+b'endstream'
        return fixtures.sample.write_pdf([
            b'<< /Type /Catalog /Pages 2 0 R >>',
            b'<< /Type /Pages /Count 2 /Kids [3 0 R 6 0 R] '+(box if inherited else b'')+b'>>',
            b'<< /Type /Page /Parent 2 0 R '+(b'' if inherited else box)+b'/Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>',
            b'<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>',stream(content),
            b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 400 300] /CropBox [0 0 400 300] /Rotate 0 /Resources << /Font << /F1 4 0 R >> >> /Contents 7 0 R >>',stream(other)])


    def pages(path):
        return json.loads(run([qpdf,'--json','--json-key=pages',path]))['pages']


    def streams(path,index):
        return [run([qpdf,'--show-object='+','.join(ref.split()[:2]),'--raw-stream-data',path]) for ref in pages(path)[index]['contents']]


    def raster(path, folder, page=1):
        output=folder/'raster.ppm'
        run([binary,'--render-page',path,page,output])
        _,size,_,pixels=output.read_bytes().split(b'\n',3)
        w,h=map(int,size.split())
        assert len(pixels)==w*h*3
        return w,h,pixels


    def compare(before,after,normalized):
        assert before[:2]==after[:2]
        width,height,old=before
        new=after[2]
        x,y,w,h=normalized
        outside=0; total=0; channels=0
        for py in range(height):
            for px in range(width):
                index=(py*width+px)*3
                if x*width-2<=px<=(x+w)*width+2 and y*height-2<=py<=(y+h)*height+2:
                    total+=sum(abs(old[index+c]-new[index+c]) for c in range(3));channels+=3
                else:
                    outside+=old[index:index+3]!=new[index:index+3]
        assert outside==0,outside
        return total/channels


    poppler_version = subprocess.run(['pdftoppm', '-v'], capture_output=True, text=True, check=True)
    report=dict(schema=1,poppler_version=(poppler_version.stdout+poppler_version.stderr).splitlines()[0],
                driver_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),scope='generated_block_geometry_only',qualified_for_secure_export=False,dPDF_secure_export_enabled=False,
                recorded_at=datetime.now(timezone.utc).isoformat(),expected_cases=32,cases=[],
                binary_version=run([binary,'--version']).decode().strip(),binary_sha256=hashlib.sha256(args.binary.read_bytes()).hexdigest(),
                pdfium_sha256=hashlib.sha256(args.pdfium.read_bytes()).hexdigest(),
                prototype_sha256=hashlib.sha256((scripts/'probe-combined-block.py').read_bytes()).hexdigest(),
                unqualified=['arbitrary documents/catalog/resource reachability','fractional boxes/UserUnit','arbitrary clipping/transparency','tagged/shared content','production worker/UI','Windows/editor/physical print acceptance'])
    try:
        with tempfile.TemporaryDirectory(prefix='dpdf-block-geometry-') as directory:
            root=Path(directory)
            mark=root/'mark.png';mark.write_bytes(fixtures.png(120,80))
            stamp=root/'stamp.png';stamp.write_bytes(fixtures.png(80,120))
            for negative in (False,True):
                for cropped in (False,True):
                    for inherited in (False,True):
                        for rotation in (0,90,180,270):
                            case=dict(negative_media=negative,cropped=cropped,inherited=inherited,rotation=rotation)
                            print('CASE '+json.dumps(case),flush=True)
                            folder=root/f'case-{len(report["cases"])}';folder.mkdir()
                            original=folder/'original.pdf';original.write_bytes(document(negative,cropped,inherited,rotation))
                            before_hash=hashlib.sha256(original.read_bytes()).hexdigest()
                            placements=[]
                            for asset,box,opacity in [(mark,[125,85,225,125],.6),(stamp,[180,75,250,125],.4)]:
                                geometry=engine.region_geometry(original,box)
                                nx,ny,nw,nh=geometry['normalized'];pw,ph=geometry['page_points']
                                placements.append(dict(page=1,image=str(asset),geometry=dict(x_mm=nx*pw*25.4/72,y_mm=ny*ph*25.4/72,width_mm=nw*pw*25.4/72,height_mm=nh*ph*25.4/72),opacity=opacity))
                            recipe=folder/'placements.json';recipe.write_text(json.dumps(placements))
                            overlay=folder/'overlay.pdf';raw=folder/'raw.pdf';final=folder/'final.pdf'
                            run([binary,'--overlay',original,overlay,recipe])
                            before=raster(overlay,folder)
                            result=engine.bake_generated(overlay,raw,region)
                            assert result['removed_objects']==5,result
                            run([qpdf,'--stream-data=preserve',original,'--pages',raw,'1',original,'2','--',final])
                            run([qpdf,'--check',final])
                            assert streams(final,1)==streams(original,1)
                            assert raster(final,folder,2)==raster(original,folder,2)
                            mae=compare(before,raster(final,folder),result['geometry']['normalized'])
                            assert mae<4,mae
                            text=json.loads(run([binary,'--probe-search',final,'BAKE_LABEL']))
                            assert text['matches']==0
                            assert json.loads(run([binary,'--probe-search',final,'OUTSIDE']))['matches']==1
                            assert len(pages(final)[0]['images'])==1
                            external=[]
                            for path in (overlay,final):
                                payload=run(['pdftoppm','-cropbox','-singlefile','-r','350','-f','1','-l','1',path])
                                magic,size,maximum,pixels=payload.split(b'\n',3);pw,ph=map(int,size.split())
                                assert magic==b'P6' and maximum==b'255' and len(pixels)==pw*ph*3
                                external.append((pw,ph,pixels))
                            poppler_mae=compare(*external,result['geometry']['normalized'])
                            assert poppler_mae<4,poppler_mae
                            text=run(['pdftotext','-f','1','-l','1','-layout','-enc','UTF-8',final,'-'])
                            assert b'BAKE_LABEL' not in text and b'BAKE_DATE' not in text and b'OUTSIDE' in text
                            removed=folder/'removed-block.pdf'
                            effective=result['geometry']['effective_pdf_region']
                            bounds=[effective[0]-.02,effective[1]-.02,effective[2]+.02,effective[3]+.02]
                            removal=engine.remove_contained(final,removed,bounds)
                            assert removal['selected']==1 and [o['type'] for o in removal['objects'] if o['intersects']]==[3]
                            rw,rh,rgb=raster(removed,folder)
                            nx,ny,nw,nh=result['geometry']['normalized']
                            assert all(rgb[(y*rw+x)*3+c]>=250 for y in range(int(ny*rh)+2,int((ny+nh)*rh)-2) for x in range(int(nx*rw)+2,int((nx+nw)*rw)-2) for c in range(3))
                            assert hashlib.sha256(original.read_bytes()).hexdigest()==before_hash
                            report['cases'].append(dict(**case,result='PASS',block_mean_abs_error=mae,poppler_350dpi_mean_abs_error=poppler_mae,removable_combined_object=1,**result))
                            print('PASS '+json.dumps(dict(**case,block_pixels=result['block_pixels'],error=mae)),flush=True)
    finally:
        engine.close()
        report['status']='PASS' if len(report['cases'])==report['expected_cases'] else 'INCOMPLETE'
        args.output.write_text(json.dumps(report,indent=2)+'\n')


if __name__ == '__main__':
    main()
