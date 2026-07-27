"""Integration tests for CarbonClient (unified API). Requires Carbon on localhost."""

import pytest
from carbon_cache import CarbonClient
from carbon_cache.exceptions import CacheNotFoundError

CACHE = "test"


@pytest.fixture(scope="module")
def client():
    with CarbonClient(
        host="localhost",
        tcp_port=5500,
        http_port=8080,
        username="admin",
        password="admin123",
    ) as c:
        c.create_cache(CACHE, eviction="ttl")
        yield c
        c.drop_cache(CACHE)


def test_ping(client: CarbonClient):
    assert client.ping() is True


def test_put_and_get_bytes(client: CarbonClient):
    client.put(CACHE, b"client_key1", b"bytes_value")
    assert client.get(CACHE, b"client_key1") == b"bytes_value"


def test_put_and_get_strings(client: CarbonClient):
    client.put(CACHE, "client_str_key", "string_value")
    assert client.get(CACHE, "client_str_key") == b"string_value"


def test_get_missing_returns_none(client: CarbonClient):
    assert client.get(CACHE, b"__missing__") is None


def test_delete(client: CarbonClient):
    client.put(CACHE, b"client_del_key", b"v")
    assert client.delete(CACHE, b"client_del_key") is True
    assert client.get(CACHE, b"client_del_key") is None


def test_delete_missing_returns_false(client: CarbonClient):
    result = client.delete(CACHE, b"__never_existed__")
    print(f"delete result for missing key: {result}")
    assert result is False


def test_create_and_drop_cache(client: CarbonClient):
    client.create_cache("client_test_cache", eviction="ttl")
    names = [c.name for c in client.list_caches()]
    assert "client_test_cache" in names
    client.drop_cache("client_test_cache")
    names = [c.name for c in client.list_caches()]
    assert "client_test_cache" not in names


def test_drop_nonexistent_cache_raises(client: CarbonClient):
    with pytest.raises(CacheNotFoundError):
        client.drop_cache("__no_such_cache__")


def test_context_manager():
    with CarbonClient(host="localhost", tcp_port=5500, http_port=8080) as c:
        assert c.ping() is True
