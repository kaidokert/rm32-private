"""Driver-disabled UART DMA CRC/sequence roundtrip sweep; never issues drive."""
import argparse
import json
from pathlib import Path
import struct
import time
import zlib
import serial

def drain(port,seconds):
    end=time.monotonic()+seconds; data=bytearray()
    while time.monotonic()<end: data.extend(port.read(8192))
    return bytes(data)

def frame(seq):
    body=b'DMS1'+struct.pack('<I',seq)+bytes((i*73+seq*19)&255 for i in range(1012))
    return body+struct.pack('<I',zlib.crc32(body))

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--port',default='COM41')
    p.add_argument('--bauds',default='115200,460800,1000000,2000000,3000000')
    p.add_argument('--tag',default='uart_dma_01')
    args=p.parse_args(); base=Path(__file__).resolve().parents[1]/'captures'/args.tag
    results=[]
    with serial.Serial(args.port,115200,timeout=.05,write_timeout=2) as port:
        for baud in map(int,args.bauds.split(',')):
            record={'baud':baud,'frames':0,'error':None};raw=bytearray(); notes=bytearray()
            try:
                port.baudrate=115200;port.write(b'off\r\n');notes.extend(drain(port,.2))
                port.write(f'uartdma{baud}\r\n'.encode())
                ack=port.read_until(b'switch_ms=200',size=512);notes.extend(ack)
                if b'DMA ready' not in ack: raise RuntimeError('missing DMA handshake')
                port.baudrate=baud;time.sleep(.25);port.reset_input_buffer()
                started=time.monotonic()
                for seq in range(32):
                    request=frame(seq);port.write(request);reply=bytearray();deadline=time.monotonic()+2
                    while len(reply)<1024 and time.monotonic()<deadline:
                        reply.extend(port.read(1024-len(reply)))
                    raw.extend(reply)
                    if len(reply)!=1024: raise RuntimeError(f'short frame {seq}: {len(reply)}')
                    if zlib.crc32(reply[:1020])!=struct.unpack('<I',reply[1020:])[0]:
                        raise RuntimeError(f'CRC mismatch {seq}')
                    expected=request[:8]+bytes(b^0xa5 for b in request[8:1020])
                    if reply[:1020]!=expected: raise RuntimeError(f'payload/sequence mismatch {seq}')
                    record['frames']+=1
                record['seconds']=time.monotonic()-started
                record['roundtrip_wire_bytes_per_s']=32*2048/record['seconds']
                port.baudrate=115200;notes.extend(drain(port,.5))
                if b'DMA done frames=32 rx_crc_bad=0 uart_error_bits_sum=0 timeout=0' not in notes:
                    raise RuntimeError('missing clean firmware completion')
            except Exception as exc:
                record['error']=str(exc)
                # Firmware exits on two-second inactivity and restores baud.
                time.sleep(2.3);port.baudrate=115200;notes.extend(drain(port,.2))
            finally:
                port.baudrate=115200
                for cmd in [b'off',b'p',b'i']:
                    port.write(cmd+b'\r\n');notes.extend(drain(port,.2))
                record['safe_readback']=(b'TIM1:moe=0 ccrA=0 ccrB=0 ccrC=0' in notes and b'en=0 nflt=' in notes)
                base.with_name(base.name+f'_{baud}.bin').write_bytes(raw)
                base.with_name(base.name+f'_{baud}.txt').write_bytes(notes)
                results.append(record);base.with_suffix('.json').write_text(json.dumps(results,indent=2))
                print(json.dumps(record),flush=True)
            if record['error'] or not record['safe_readback']: break

if __name__=='__main__':main()
