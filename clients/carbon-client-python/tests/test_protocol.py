import struct
import pytest
from carbon_cache._protocol import (
    CMD_PING, CMD_PUT, CMD_GET, CMD_DELETE,
    RESP_PONG, RESP_OK, RESP_VALUE, RESP_NOT_FOUND, RESP_ERROR,
    wrap_frame, build_ping, build_put, build_get, build_delete,
    parse_response, decode_value_payload, decode_error_payload,
)

_U32 = struct.Struct(">I")


def _unpack_u32(data: bytes, offset: int) -> tuple[int, int]:
    (val,) = _U32.unpack(data[offset : offset + 4])
    return val, offset + 4


class TestWrapFrame:
    def test_prepends_4_byte_length(self):
        payload = b"hello"
        framed = wrap_frame(payload)
        assert len(framed) == 4 + len(payload)
        (length,) = _U32.unpack(framed[:4])
        assert length == len(payload)
        assert framed[4:] == payload

    def test_empty_payload(self):
        framed = wrap_frame(b"")
        (length,) = _U32.unpack(framed[:4])
        assert length == 0

    def test_length_is_big_endian(self):
        payload = b"x" * 256
        framed = wrap_frame(payload)
        assert framed[:4] == b"\x00\x00\x01\x00"


class TestBuildPing:
    def test_opcode_and_frame(self):
        frame = build_ping()
        (length,) = _U32.unpack(frame[:4])
        assert length == 1
        assert frame[4] == CMD_PING


class TestBuildPut:
    def test_structure(self):
        frame = build_put("my_cache", b"key1", b"value1")
        (frame_len,) = _U32.unpack(frame[:4])
        body = frame[4:]
        assert len(body) == frame_len

        assert body[0] == CMD_PUT
        pos = 1

        cache_len, pos = _unpack_u32(body, pos)
        cache_name = body[pos : pos + cache_len]
        pos += cache_len
        assert cache_name == b"my_cache"

        key_len, pos = _unpack_u32(body, pos)
        val_len, pos = _unpack_u32(body, pos)
        key = body[pos : pos + key_len]
        pos += key_len
        value = body[pos : pos + val_len]

        assert key == b"key1"
        assert value == b"value1"

    def test_unicode_cache_name(self):
        frame = build_put("кэш", b"k", b"v")
        body = frame[4:]
        assert body[0] == CMD_PUT
        cache_len, _ = _unpack_u32(body, 1)
        cache_bytes = body[5 : 5 + cache_len]
        assert cache_bytes.decode() == "кэш"

    def test_binary_key_and_value(self):
        frame = build_put("c", b"\x00\xff\xfe", b"\xde\xad\xbe\xef")
        body = frame[4:]
        cache_len, pos = _unpack_u32(body, 1)
        pos += cache_len
        key_len, pos = _unpack_u32(body, pos)
        val_len, pos = _unpack_u32(body, pos)
        key = body[pos : pos + key_len]
        pos += key_len
        value = body[pos : pos + val_len]
        assert key == b"\x00\xff\xfe"
        assert value == b"\xde\xad\xbe\xef"


class TestBuildGet:
    def test_structure(self):
        frame = build_get("cache1", b"mykey")
        body = frame[4:]
        assert body[0] == CMD_GET
        cache_len, pos = _unpack_u32(body, 1)
        cache_name = body[pos : pos + cache_len]
        pos += cache_len
        key_len, pos = _unpack_u32(body, pos)
        key = body[pos : pos + key_len]
        assert cache_name == b"cache1"
        assert key == b"mykey"


class TestBuildDelete:
    def test_structure(self):
        frame = build_delete("cache1", b"mykey")
        body = frame[4:]
        assert body[0] == CMD_DELETE
        cache_len, pos = _unpack_u32(body, 1)
        pos += cache_len
        key_len, pos = _unpack_u32(body, pos)
        key = body[pos : pos + key_len]
        assert key == b"mykey"


class TestParseResponse:
    def test_single_byte_responses(self):
        for opcode in (RESP_PONG, RESP_OK, RESP_NOT_FOUND):
            code, payload = parse_response(bytes([opcode]))
            assert code == opcode
            assert payload == b""

    def test_value_response(self):
        value = b"hello world"
        data = bytes([RESP_VALUE]) + _U32.pack(len(value)) + value
        code, payload = parse_response(data)
        assert code == RESP_VALUE
        assert decode_value_payload(payload) == value

    def test_error_response(self):
        msg = "cache not found"
        msg_b = msg.encode()
        data = bytes([RESP_ERROR]) + _U32.pack(len(msg_b)) + msg_b
        code, payload = parse_response(data)
        assert code == RESP_ERROR
        assert decode_error_payload(payload) == msg

    def test_empty_data_raises(self):
        with pytest.raises(ConnectionError):
            parse_response(b"")
