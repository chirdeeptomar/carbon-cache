"""Integration tests for TcpTransport. Requires Carbon running on localhost:5500."""

import pytest
from carbon_cache._http import HttpTransport
from carbon_cache._tcp import TcpTransport

CACHE = "test"


@pytest.fixture(scope="module")
def tcp():
    http = HttpTransport(host="localhost", port=8080,
                         username="admin", password="admin123")
    http.create_cache(CACHE, eviction="ttl")
    t = TcpTransport(host="localhost", port=5500)
    yield t
    t.close()
    http.drop_cache(CACHE)


def test_ping(tcp):
    assert tcp.ping() is True


def test_put_and_get(tcp):
    tcp.put(CACHE, b"tcp_key1", b"hello")
    val = tcp.get(CACHE, b"tcp_key1")
    assert val == b"hello"


def test_get_missing_key(tcp):
    val = tcp.get(CACHE, b"__no_such_key__")
    assert val is None


def test_put_binary_value(tcp):
    data = bytes(range(256))
    tcp.put(CACHE, b"bin_key", data)
    assert tcp.get(CACHE, b"bin_key") == data


def test_put_unicode_value(tcp):
    value = "こんにちは世界".encode()
    tcp.put(CACHE, b"unicode_key", value)
    assert tcp.get(CACHE, b"unicode_key") == value


def test_delete_existing_key(tcp):
    tcp.put(CACHE, b"del_key", b"to_delete")
    deleted = tcp.delete(CACHE, b"del_key")
    assert deleted is True
    assert tcp.get(CACHE, b"del_key") is None


def test_delete_missing_key(tcp):
    result = tcp.delete(CACHE, b"__never_existed__")
    assert result is False


def test_overwrite_key(tcp):
    tcp.put(CACHE, b"overwrite_key", b"first")
    tcp.put(CACHE, b"overwrite_key", b"second")
    assert tcp.get(CACHE, b"overwrite_key") == b"second"
