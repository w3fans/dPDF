#!/usr/bin/env python3
"""Generated native XDND file-drop acceptance on a private authenticated X display."""
import argparse
import ctypes as C
import ctypes.util
import hashlib
import importlib.util
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time

SCRIPTS = Path(__file__).resolve().parent

def module(name, filename):
    spec = importlib.util.spec_from_file_location(name, SCRIPTS / filename)
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value

U = C.c_ulong
P = C.c_void_p
class Data(C.Union):
    _fields_ = [('b', C.c_char*20), ('s', C.c_short*10), ('l', C.c_long*5)]
class Client(C.Structure):
    _fields_ = [('type', C.c_int), ('serial', U), ('send_event', C.c_int), ('display', P),
                ('window', U), ('message_type', U), ('format', C.c_int), ('data', Data)]
class Request(C.Structure):
    _fields_ = [('type', C.c_int), ('serial', U), ('send_event', C.c_int), ('display', P),
                ('owner', U), ('requestor', U), ('selection', U), ('target', U), ('property', U), ('time', U)]
class Notify(C.Structure):
    _fields_ = [('type', C.c_int), ('serial', U), ('send_event', C.c_int), ('display', P),
                ('requestor', U), ('selection', U), ('target', U), ('property', U), ('time', U)]
class Event(C.Union):
    _fields_ = [('type', C.c_int), ('client', Client), ('request', Request), ('notify', Notify), ('pad', C.c_long*24)]

class X11:
    def __init__(self):
        self.x = C.CDLL(C.util.find_library('X11'))
        for name, result, arguments in [
            ('XOpenDisplay',P,[C.c_char_p]),('XDefaultRootWindow',U,[P]),
            ('XInternAtom',U,[P,C.c_char_p,C.c_int]),
            ('XCreateSimpleWindow',U,[P,U,C.c_int,C.c_int,C.c_uint,C.c_uint,C.c_uint,U,U]),
            ('XQueryTree',C.c_int,[P,U,C.POINTER(U),C.POINTER(U),C.POINTER(C.POINTER(U)),C.POINTER(C.c_uint)]),
            ('XGetWindowProperty',C.c_int,[P,U,U,C.c_long,C.c_long,C.c_int,U,C.POINTER(U),C.POINTER(C.c_int),C.POINTER(U),C.POINTER(U),C.POINTER(P)]),
            ('XChangeProperty',C.c_int,[P,U,U,U,C.c_int,C.c_int,P,C.c_int]),
            ('XSetSelectionOwner',C.c_int,[P,U,U,U]), ('XGetSelectionOwner',U,[P,U]),
            ('XSendEvent',C.c_int,[P,U,C.c_int,C.c_long,C.POINTER(Event)]),
            ('XPending',C.c_int,[P]),('XNextEvent',C.c_int,[P,C.POINTER(Event)]),
            ('XFlush',C.c_int,[P]),('XFree',C.c_int,[P]),
            ('XStringToKeysym',U,[C.c_char_p]),('XKeysymToKeycode',C.c_ubyte,[P,U]),
            ('XSetInputFocus',C.c_int,[P,U,C.c_int,U]),
            ('XDestroyWindow',C.c_int,[P,U]),('XCloseDisplay',C.c_int,[P])]:
            fn=getattr(self.x,name);fn.restype=result;fn.argtypes=arguments
        self.display=self.x.XOpenDisplay(None)
        if not self.display: raise RuntimeError('Private X display is unavailable')
        self.root=self.x.XDefaultRootWindow(self.display)
        self.source=self.x.XCreateSimpleWindow(self.display,self.root,0,0,1,1,0,0,0)
        self.atoms={name:self.x.XInternAtom(self.display,name.encode(),False) for name in
                    ['_NET_WM_PID','_NET_WM_NAME','WM_CLASS','XdndAware','XdndSelection','XdndEnter','XdndPosition','XdndStatus','XdndDrop','XdndFinished','XdndLeave','XdndActionCopy','text/uri-list']}

    def property(self, window, name):
        kind=U();fmt=C.c_int();count=U();remaining=U();data=P()
        result=self.x.XGetWindowProperty(self.display,window,self.atoms[name],0,4096,False,0,
                                       C.byref(kind),C.byref(fmt),C.byref(count),C.byref(remaining),C.byref(data))
        try:
            if result or not data: return None
            if fmt.value==32: return list(C.cast(data,C.POINTER(U))[:count.value])
            if fmt.value==8: return C.string_at(data,count.value).decode('utf-8',errors='replace')
            return None
        finally:
            if data: self.x.XFree(data)

    def find_window(self,pid):
        root=U();parent=U();children=C.POINTER(U)();count=C.c_uint()
        self.x.XQueryTree(self.display,self.root,C.byref(root),C.byref(parent),C.byref(children),C.byref(count))
        try:
            matches=[w for w in children[:count.value] if self.property(w,'_NET_WM_PID')==[pid]]
            return matches[0] if len(matches)==1 else None
        finally:
            if children: self.x.XFree(children)

    def send(self,window,kind,values):
        event=Event();event.client=Client(type=33,display=self.display,window=window,message_type=self.atoms[kind],format=32)
        for i,value in enumerate(values): event.client.data.l[i]=value
        if not self.x.XSendEvent(self.display,window,False,0,C.byref(event)): raise RuntimeError('Cannot send '+kind)
        self.x.XFlush(self.display)

    def drop(self,window,paths,cancel=False):
        payload=('\r\n'.join(p.as_uri() for p in paths)+'\r\n').encode()
        self.x.XSetSelectionOwner(self.display,self.atoms['XdndSelection'],self.source,0)
        assert self.x.XGetSelectionOwner(self.display,self.atoms['XdndSelection'])==self.source
        self.send(window,'XdndEnter',[self.source,5<<24,self.atoms['text/uri-list'],0,0])
        self.send(window,'XdndPosition',[self.source,0,(600<<16)|400,0,self.atoms['XdndActionCopy']])
        serviced=False;accepted=False;dropped=False;finished=False;drop_at=None
        deadline=time.monotonic()+10
        while time.monotonic()<deadline:
            while self.x.XPending(self.display):
                event=Event();self.x.XNextEvent(self.display,C.byref(event))
                if event.type==30:
                    request=event.request
                    assert request.owner==self.source and request.requestor==window
                    assert request.selection==self.atoms['XdndSelection'] and request.target==self.atoms['text/uri-list']
                    prop=request.property or request.target
                    buffer=C.create_string_buffer(payload)
                    self.x.XChangeProperty(self.display,request.requestor,prop,request.target,8,0,buffer,len(payload))
                    reply=Event();reply.notify=Notify(type=31,display=self.display,requestor=request.requestor,selection=request.selection,target=request.target,property=prop,time=request.time)
                    self.x.XSendEvent(self.display,request.requestor,False,0,C.byref(reply));self.x.XFlush(self.display)
                    serviced=True;drop_at=time.monotonic()+.2
                elif event.type==33 and event.client.message_type==self.atoms['XdndStatus']:
                    assert event.client.data.l[0]==window
                    accepted=bool(event.client.data.l[1]&1)
                elif event.type==33 and event.client.message_type==self.atoms['XdndFinished']:
                    assert event.client.data.l[0]==window and event.client.data.l[1]&1
                    finished=True
            if serviced and accepted and not dropped and time.monotonic()>=drop_at:
                if cancel:
                    self.send(window,'XdndLeave',[self.source,0,0,0,0])
                    return
                self.send(window,'XdndDrop',[self.source,0,0,0,0]);dropped=True
            if finished: return
            time.sleep(.01)
        raise RuntimeError(f'Native XDND handshake timed out: selection={serviced}, accepted={accepted}, dropped={dropped}')

    def wait_title(self,window,title,process,seconds=12):
        deadline=time.monotonic()+seconds
        while time.monotonic()<deadline:
            if process.poll() is not None: raise RuntimeError('Reader exited during native drop')
            if self.property(window,'_NET_WM_NAME')==title: return
            time.sleep(.05)
        raise RuntimeError('Native file drop did not open the expected document; window title: '+repr(self.property(window,'_NET_WM_NAME')))

    def next_tab(self,window):
        # Genuine key events stay confined to this script's private X server.
        xtst=C.CDLL(C.util.find_library('Xtst'))
        xtst.XTestFakeKeyEvent.argtypes=[P,C.c_uint,C.c_int,U]
        xtst.XTestFakeKeyEvent.restype=C.c_int
        keys=[self.x.XKeysymToKeycode(self.display,self.x.XStringToKeysym(name))
              for name in (b'Control_L',b'Tab')]
        assert all(keys)
        self.x.XSetInputFocus(self.display,window,2,0)
        for key,pressed in [(keys[0],True),(keys[1],True),(keys[1],False),(keys[0],False)]:
            assert xtst.XTestFakeKeyEvent(self.display,key,pressed,0)
        self.x.XFlush(self.display)

    def close(self):
        self.x.XDestroyWindow(self.display,self.source);self.x.XCloseDisplay(self.display)

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary',type=Path,required=True);parser.add_argument('--pdfium',type=Path,required=True)
    parser.add_argument('--xvfb',default='Xvfb');parser.add_argument('--inside',action='store_true');parser.add_argument('--folder',type=Path)
    args=parser.parse_args()
    if os.name!='posix': parser.error('This native protocol diagnostic requires Linux X11')
    if not args.inside:
        with tempfile.TemporaryDirectory(prefix='dpdf-native-drop-') as d:
            folder=Path(d)
            with module('private','private-display.py').display(folder,args.xvfb) as env:
                env['DPDF_PDFIUM_LIBRARY']=str(args.pdfium.resolve())
                subprocess.run([sys.executable,str(Path(__file__).resolve()),'--inside','--folder',str(folder),'--binary',str(args.binary.resolve()),'--pdfium',str(args.pdfium.resolve())],env=env,check=True,timeout=65)
        return
    sample=module('sample','make-sample.py');paths=[args.folder/'Pogodba ŽČŠ 100% #one.pdf',args.folder/'second contract.pdf']
    for i,path in enumerate(paths): path.write_bytes(sample.make_pdf(50+i))
    digests=[hashlib.sha256(p.read_bytes()).digest() for p in paths]
    x=X11()
    with (args.folder/'reader.log').open('wb') as log:
        process=subprocess.Popen([str(args.binary)],stdout=log,stderr=log)
        try:
            deadline=time.monotonic()+12;window=None
            while not window and time.monotonic()<deadline:
                window=x.find_window(process.pid);time.sleep(.05)
            assert window and x.property(window,'XdndAware'), 'Reader did not advertise native XDND'
            assert 'net.dpdf.dPDF' in x.property(window,'WM_CLASS').split('\0'), 'Native window does not match its desktop application identity'
            x.wait_title(window,'dPDF — Reader preview',process)
            x.drop(window,paths,cancel=True);time.sleep(.25)
            x.wait_title(window,'dPDF — Reader preview',process,seconds=1)
            x.drop(window,[paths[0],paths[0],paths[1]])
            x.wait_title(window,str(paths[-1])+' — dPDF',process)
            x.next_tab(window)
            x.wait_title(window,str(paths[0])+' — dPDF',process)
            x.next_tab(window)
            x.wait_title(window,str(paths[-1])+' — dPDF',process)
            assert digests==[hashlib.sha256(p.read_bytes()).digest() for p in paths]
            print('PASS: native XDND hover/cancel and multiple Unicode file drops open separate keyboard-switchable tabs without modifying generated inputs')
        except Exception:
            print((args.folder/'reader.log').read_text(errors='replace')[-2000:]);raise
        finally:
            process.terminate()
            try: process.wait(timeout=5)
            except subprocess.TimeoutExpired: process.kill();process.wait()
            x.close()

if __name__=='__main__':
    main()
