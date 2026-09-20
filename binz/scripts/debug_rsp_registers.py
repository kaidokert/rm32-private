"""Read halted ARM registers via an already-running local probe-rs GDB stub.

Bench diagnostic only: use with driver verified disabled. Leaves CPU halted.
No memory writes, reset, or motor commands.
"""
import socket
import struct
import argparse


def receive(sock):
    while True:
        byte = sock.recv(1)
        if not byte:
            raise EOFError("GDB disconnected")
        if byte == b"$":
            break
    payload = bytearray()
    while True:
        byte = sock.recv(1)
        if byte == b"#":
            break
        if not byte:
            raise EOFError("GDB disconnected")
        payload.extend(byte)
    checksum = b""
    while len(checksum) < 2:
        checksum += sock.recv(2 - len(checksum))
    if sum(payload) % 256 != int(checksum, 16):
        raise ValueError("GDB checksum mismatch")
    sock.sendall(b"+")
    decoded = bytearray()
    index = 0
    while index < len(payload):
        byte = payload[index]
        if byte == ord("*"):
            index += 1
            decoded.extend([decoded[-1]] * (payload[index] - 29))
        elif byte == ord("}"):
            index += 1
            decoded.append(payload[index] ^ 0x20)
        else:
            decoded.append(byte)
        index += 1
    return bytes(decoded)


parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--host', choices=['127.0.0.1', '::1'], default='127.0.0.1')
args = parser.parse_args()
with socket.create_connection((args.host, 1337), timeout=3) as sock:
    sock.settimeout(3)
    sock.sendall(b"$?#3f")
    print("halt:", receive(sock).decode())
    sock.sendall(b"$g#67")
    raw = receive(sock)
    print("register_packet:", raw.decode())
    regs = struct.unpack("<" + "I" * (len(raw) // 8), bytes.fromhex(raw.decode()))
    for index, value in enumerate(regs):
        name = ("sp", "lr", "pc")[index - 13] if 13 <= index <= 15 else f"r{index}"
        print(f"{name}=0x{value:08x}")
