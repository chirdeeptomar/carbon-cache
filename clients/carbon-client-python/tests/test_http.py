"""Integration tests for HttpTransport. Requires Carbon running on localhost:8080."""

import pytest
from carbon_cache._http import HttpTransport
from carbon_cache.exceptions import CacheNotFoundError


@pytest.fixture(scope="module")
def http():
    return HttpTransport(
        host="localhost", port=8080, username="admin", password="admin123"
    )


def test_health(http):
    msg = http.health()
    assert msg == "OK"


def test_create_and_drop_cache(http):
    http.create_cache("http_test_cache", eviction="ttl")
    caches = http.list_caches()
    names = [c.name for c in caches]
    assert "http_test_cache" in names

    http.drop_cache("http_test_cache")
    caches = http.list_caches()
    names = [c.name for c in caches]
    assert "http_test_cache" not in names


def test_create_cache_with_ttl(http):
    http.create_cache("ttl_cache", eviction="ttl", default_ttl_ms=60_000)
    info = http.describe_cache("ttl_cache")
    assert info.name == "ttl_cache"
    http.drop_cache("ttl_cache")


def test_list_caches_returns_list(http):
    caches = http.list_caches()
    assert isinstance(caches, list)


def test_drop_nonexistent_cache_raises(http):
    with pytest.raises(CacheNotFoundError):
        http.drop_cache("__no_such_cache__")


def test_describe_nonexistent_cache_raises(http):
    with pytest.raises(CacheNotFoundError):
        http.describe_cache("__no_such_cache__")
