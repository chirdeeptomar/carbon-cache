"""
TCP wire protocol for Carbon cache server.

Frame format: every message is wrapped with a 4-byte big-endian length prefix
(matching Tokio's LengthDelimitedCodec on the server side).

Request opcodes:
  0x00 PING
  0x01 PUT   [u32 cache_len][cache_name][u32 key_len][u32 val_len][key][value]
  0x02 GET   [u32 cache_len][cache_name][u32 key_len][key]
  0x03 DEL   [u32 cache_len][cache_name][u32 key_len][key]

Response opcodes:
  0x00 PONG
  0x01 OK
  0x02 VALUE      [u32 val_len][value]
  0x03 NOT_FOUND
  0x04 ERROR      [u32 msg_len][msg_utf8]

All multi-byte integers are big-endian. Cache names / error messages are UTF-8.
Keys and values are raw bytes.
"""

from __future__ import annotations

import socket
import struct

CMD_PING = 0x00
CMD_PUT = 0x01
CMD_GET = 0x02
CMD_DELETE = 0x03

RESP_PONG = 0x00
RESP_OK = 0x01
RESP_VALUE = 0x02
RESP_NOT_FOUND = 0x03
RESP_ERROR = 0x04

_U32 = struct.Struct(">I")  # big-endian unsigned 32-bit int


def _u32(n: int) -> bytes:
    return _U32.pack(n)


def wrap_frame(payload: bytes) -> bytes:
    return _u32(len(payload)) + payload


def read_frame(sock: socket.socket) -> bytes:
    raw_len = _recv_exact(sock, 4)
    (length,) = _U32.unpack(raw_len)
    return _recv_exact(sock, length)


def _recv_exact(sock: socket.socket, n: int) -> bytes:
    buf = bytearray()
    while len(buf) < n:
        chunk = sock.recv(n - len(buf))
        if not chunk:
            raise ConnectionError("Connection closed by server")
        buf.extend(chunk)
    return bytes(buf)


def build_ping() -> bytes:
    return wrap_frame(bytes([CMD_PING]))


def build_put(cache: str, key: bytes, value: bytes) -> bytes:
    cache_b = cache.encode()
    payload = (
        bytes([CMD_PUT])
        + _u32(len(cache_b))
        + cache_b
        + _u32(len(key))
        + _u32(len(value))
        + key
        + value
    )
    return wrap_frame(payload)


def build_get(cache: str, key: bytes) -> bytes:
    cache_b = cache.encode()
    payload = (
        bytes([CMD_GET])
        + _u32(len(cache_b))
        + cache_b
        + _u32(len(key))
        + key
    )
    return wrap_frame(payload)


def build_delete(cache: str, key: bytes) -> bytes:
    cache_b = cache.encode()
    payload = (
        bytes([CMD_DELETE])
        + _u32(len(cache_b))
        + cache_b
        + _u32(len(key))
        + key
    )
    return wrap_frame(payload)


def parse_response(data: bytes) -> tuple[int, bytes]:
    """Return (opcode, remaining_payload)."""
    if not data:
        raise ConnectionError("Empty response from server")
    return data[0], data[1:]


def decode_value_payload(payload: bytes) -> bytes:
    (length,) = _U32.unpack(payload[:4])
    return payload[4 : 4 + length]


def decode_error_payload(payload: bytes) -> str:
    (length,) = _U32.unpack(payload[:4])
    return payload[4 : 4 + length].decode()
