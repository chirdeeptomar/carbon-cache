from __future__ import annotations

import socket
import threading

from . import _protocol as proto
from .exceptions import ConnectionError, ServerError


class TcpTransport:
    """Persistent TCP connection to Carbon. Thread-safe."""

    def __init__(self, host: str = "localhost", port: int = 5500) -> None:
        self._host = host
        self._port = port
        self._sock: socket.socket | None = None
        self._lock = threading.Lock()
        self.connect()

    def connect(self) -> None:
        try:
            sock = socket.create_connection((self._host, self._port), timeout=10)
            sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
            sock.settimeout(30)
            self._sock = sock
        except OSError as e:
            raise ConnectionError(
                f"Cannot connect to {self._host}:{self._port}: {e}"
            ) from e

    def close(self) -> None:
        with self._lock:
            if self._sock:
                try:
                    self._sock.close()
                except OSError:
                    pass
                self._sock = None

    def ping(self) -> bool:
        data = self._send(proto.build_ping())
        opcode, _ = proto.parse_response(data)
        return opcode == proto.RESP_PONG

    def put(self, cache: str, key: bytes, value: bytes) -> None:
        data = self._send(proto.build_put(cache, key, value))
        opcode, payload = proto.parse_response(data)
        if opcode == proto.RESP_ERROR:
            raise ServerError(proto.decode_error_payload(payload))
        if opcode != proto.RESP_OK:
            raise ServerError(f"Unexpected response opcode: {opcode}")

    def get(self, cache: str, key: bytes) -> bytes | None:
        data = self._send(proto.build_get(cache, key))
        opcode, payload = proto.parse_response(data)
        print(f"get response: opcode={opcode}, payload={payload!r}")
        if opcode == proto.RESP_NOT_FOUND:
            return None
        if opcode == proto.RESP_VALUE:
            return proto.decode_value_payload(payload)
        if opcode == proto.RESP_ERROR:
            raise ServerError(proto.decode_error_payload(payload))
        raise ServerError(f"Unexpected response opcode: {opcode}")

    def delete(self, cache: str, key: bytes) -> bool:
        data = self._send(proto.build_delete(cache, key))
        opcode, payload = proto.parse_response(data)
        print(f"delete response: opcode={opcode}, payload={payload!r}")
        if opcode == proto.RESP_OK:
            return True
        if opcode == proto.RESP_NOT_FOUND:
            return False
        if opcode == proto.RESP_ERROR:
            raise ServerError(proto.decode_error_payload(payload))
        raise ServerError(f"Unexpected response opcode: {opcode:#04x}")

    def _send(self, frame: bytes) -> bytes:
        with self._lock:
            try:
                return self._send_once(frame)
            except BrokenPipeError:
                # reconnect once on stale connection
                self.connect()
                return self._send_once(frame)

    def _send_once(self, frame: bytes) -> bytes:
        if self._sock is None:
            raise ConnectionError("Not connected")
        try:
            self._sock.sendall(frame)
            return proto.read_frame(self._sock)
        except OSError as e:
            raise ConnectionError(f"TCP error: {e}") from e
