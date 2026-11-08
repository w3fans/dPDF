#!/usr/bin/env python3
"""Native PDF review revisions, visibility, clean copy and untouched-content checks."""
import argparse
import copy
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile


def module(name, filename):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(filename))
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=Path('target/release/dpdf'))
    parser.add_argument('--pdfium', type=Path, required=True)
    parser.add_argument('--qpdf', type=Path, required=True)
    args = parser.parse_args()
    sample = module('sample', 'make-sample.py')
    binary, qpdf = str(args.binary.resolve()), str(args.qpdf.resolve())
    env = dict(os.environ, DPDF_PDFIUM_LIBRARY=str(args.pdfium.resolve()), DPDF_QPDF_BINARY=qpdf)

    def run(command, valid=True):
        result = subprocess.run(command, env=env, capture_output=True, timeout=60)
        if valid and result.returncode:
            raise AssertionError(result.stderr.decode(errors='replace'))
        return result

    def pdf_json(path):
        return json.loads(run([qpdf, '--json', '--json-stream-data=none', str(path)]).stdout)

    def native(path):
        data = pdf_json(path)
        objects = data['qpdf'][1]
        result = []
        for page in data['pages']:
            entries = objects['obj:' + page['object']]['value'].get('/Annots', [])
            if isinstance(entries, str):
                entries = objects['obj:' + entries]['value']
            result.append([objects['obj:' + item]['value'] if isinstance(item, str) else item for item in entries])
        return result

    def raw_streams(path):
        return [[run([qpdf, '--show-object=' + ','.join(ref.split()[:2]), '--raw-stream-data', str(path)]).stdout
                 for ref in page['contents']] for page in pdf_json(path)['pages']]

    with tempfile.TemporaryDirectory(prefix='dpdf-review-') as directory:
        folder = Path(directory)
        recipe = folder / 'review.json'
        original = folder / 'source.pdf'
        original.write_bytes(sample.make_pdf(10))
        before = hashlib.sha256(original.read_bytes()).digest()
        workspace = 'aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee'
        rect = lambda x, y, w, h: dict(x=x, y=y, width=w, height=h)
        model = dict(schema_version=1, workspace=workspace,
                     revisions=[dict(id=i, label=f'Revision {i}') for i in (1, 2, 3)],
                     current=2, visible=True, filter='all', remarks=[
                         dict(id=1, page=0, revision=1, kind='note', bounds=rect(.72,.7,.04,.04), text='Žig čšž review note'),
                         dict(id=2, page=0, revision=2, kind='highlight', bounds=rect(.1,.4,.4,.05), text='Second revision highlight'),
                         dict(id=3, page=1, revision=2, kind='underline', bounds=rect(.1,.4,.4,.05), text='Underline review'),
                     ])

        def export(source, name, value, mode='current', valid=True):
            recipe.write_text(json.dumps(value), encoding='utf-8')
            output = folder / name
            result = run([binary, '--review', 'export', str(source), str(output), str(recipe), mode], valid)
            if valid:
                run([qpdf, '--check', str(output)])
            return output, result

        def read(path):
            return json.loads(run([binary, '--review', 'inspect', str(path)]).stdout)

        def raster(path, page=1):
            output = folder / 'frame.ppm'
            run([binary, '--render-page', str(path), str(page), str(output)])
            _, size, _, pixels = output.read_bytes().split(b'\n', 3)
            width, height = map(int, size.split())
            return width, height, pixels

        visible, _ = export(original, 'visible.pdf', model, 'visible')
        assert raw_streams(visible) == raw_streams(original)
        assert [a['/Subtype'] for p in native(visible) for a in p] == ['/Text', '/Highlight', '/Underline']
        restored = read(visible)
        assert restored['workspace'] == workspace and restored['revisions'] == model['revisions']
        assert [r['text'] for r in restored['remarks']] == [r['text'] for r in model['remarks']]
        assert raster(visible)[2] != raster(original)[2]
        assert raster(visible, 2)[2] != raster(original, 2)[2]
        assert raster(visible, 10) == raster(original, 10)
        print('PASS: native note/highlight/underline, Unicode revision metadata and exact page content streams')

        hidden_model = copy.deepcopy(restored)
        hidden_model['visible'] = False
        hidden, _ = export(visible, 'hidden.pdf', hidden_model)
        assert all(a['/F'] & 2 for p in native(hidden) for a in p)
        assert raster(hidden) == raster(original)
        assert raster(hidden, 2) == raster(original, 2)
        assert not read(hidden)['visible']
        current_model=copy.deepcopy(restored)
        current_model['schema_version']=2
        current_model['filter']='current'
        current_model['current']=2
        current,_=export(visible,'current-revision.pdf',current_model)
        assert read(current)['filter']=='current'
        assert [bool(a['/F'] & 2) for p in native(current) for a in p]==[True,False,False]
        subset_model = copy.deepcopy(restored)
        subset_model['filter'] = {'selected': [2]}
        subset, _ = export(visible, 'subset.pdf', subset_model)
        entries = [a for p in native(subset) for a in p]
        assert entries[0]['/F'] & 2 and all(not a['/F'] & 2 for a in entries[1:])
        assert read(subset)['filter'] == {'selected': [2]}
        clean, _ = export(subset, 'clean.pdf', read(subset), 'clean')
        assert read(clean) is None and all(not page for page in native(clean))
        assert raw_streams(clean) == raw_streams(original) and raster(clean) == raster(original)
        assert 'Žig' not in json.dumps(pdf_json(clean), ensure_ascii=False)
        print('PASS: hidden/current-revision filters retain remarks; clean copy removes owned review and metadata')

        # Crop/rotation: visible geometry returns through PDF coordinates; hidden pixels remain exact.
        for cropped in (False, True):
            for angle in (0, 90, 180, 270):
                source = folder / f'page-{int(cropped)}-{angle}.pdf'
                source.write_bytes(sample.make_pdf(1, rotation=angle, cropped=cropped))
                value = copy.deepcopy(model)
                value['remarks'] = [dict(id=1,page=0,revision=2,kind='highlight',bounds=rect(.15,.4,.5,.08),text='Geometry')]
                marked, _ = export(source, 'geometry.pdf', value)
                record = read(marked)['remarks'][0]['bounds']
                assert all(abs(record[key]-value['remarks'][0]['bounds'][key]) < .00001 for key in record), (angle, cropped, record)
                width, height, pixels = raster(marked)
                _, _, plain = raster(source)
                assert pixels != plain
                for y in range(height):
                    spans = [(0,width)] if not .4*height-3 <= y <= .48*height+3 else [(0,max(0,int(.15*width)-3)),(min(width,int(.65*width)+4),width)]
                    for left,right in spans:
                        lo,hi=(y*width+left)*3,(y*width+right)*3
                        assert pixels[lo:hi] == plain[lo:hi], (angle,cropped,y)
                value['visible'] = False
                invisible, _ = export(marked, 'geometry-hidden.pdf', value)
                assert raster(invisible) == raster(source)
        print('PASS: native markup geometry and visibility at all intrinsic quarter turns and crop offsets')

        # Retained comment object IDs keep external replies/popups. Clean retires the entire owned thread.
        data = pdf_json(visible)
        objects = data['qpdf'][1]
        page_ref = data['pages'][0]['object']
        page = objects['obj:' + page_ref]['value']
        note_ref = page['/Annots'][0]
        note = objects['obj:' + note_ref]['value']
        next_id = data['qpdf'][0]['maxobjectid'] + 1
        popup, reply, foreign = [f'{next_id+i} 0 R' for i in range(3)]
        note['/Popup'] = popup
        page['/Annots'] += [popup, reply, foreign]
        changes = {
            'obj:' + page_ref: {'value': page}, 'obj:' + note_ref: {'value': note},
            'obj:' + popup: {'value': {'/Type':'/Annot','/Subtype':'/Popup','/Parent':note_ref,'/Rect':[0,0,50,50]}},
            'obj:' + reply: {'value': {'/Type':'/Annot','/Subtype':'/Text','/IRT':note_ref,'/Rect':[50,50,70,70],'/Contents':'u:Private reply in the review thread','/NM':'u:external-reply'}},
            'obj:' + foreign: {'value': {'/Type':'/Annot','/Subtype':'/Text','/Rect':[20,20,40,40],'/Contents':'u:Foreign annotation must survive','/NM':'u:foreign-note'}},
        }
        patch = folder / 'foreign.json'
        patch.write_text(json.dumps({'qpdf':[{'jsonversion':2},changes]}))
        threaded = folder / 'threaded.pdf'
        run([qpdf, str(visible), '--update-from-json='+str(patch), str(threaded)])
        kept, _ = export(threaded, 'thread-kept.pdf', read(threaded))
        assert any(a.get('/Contents') == 'u:Private reply in the review thread' for p in native(kept) for a in p)
        wiped, _ = export(kept, 'thread-clean.pdf', read(kept), 'clean')
        remaining = [a for p in native(wiped) for a in p]
        assert len(remaining) == 1 and remaining[0]['/Contents'] == 'u:Foreign annotation must survive'
        assert 'Private reply in the review thread' not in json.dumps(pdf_json(wiped))
        print('PASS: native popup/reply links survive retained comments; clean removes owned threads and keeps foreign annotations')

        prior = visible.read_bytes()
        invalid = copy.deepcopy(model)
        invalid['remarks'][0]['page'] = 999
        _, result = export(original, 'visible.pdf', invalid, valid=False)
        assert result.returncode and visible.read_bytes() == prior
        recipe.write_text(json.dumps(model))
        assert run([binary,'--review','export',str(original),str(original),str(recipe),'visible'],False).returncode
        alias = folder / 'source-hardlink.pdf'
        os.link(original, alias)
        assert run([binary,'--review','export',str(original),str(alias),str(recipe),'visible'],False).returncode
        assert hashlib.sha256(original.read_bytes()).digest() == before
        print('PASS: invalid review and source-alias destinations preserve source and prior output')


if __name__ == '__main__':
    main()
