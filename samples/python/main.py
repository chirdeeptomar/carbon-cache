"""
Carbon Python client sample.

Demonstrates:
  - Connecting to a Carbon server
  - Basic put / get / delete operations
  - Cache management (create / list / drop)
  - Context manager usage

Prerequisites: Carbon server running on localhost:5500 (TCP) and localhost:8080 (HTTP).
  Start it with: cargo run -p carbon-server
"""

from carbon_cache import CarbonClient
from carbon_cache.exceptions import CacheNotFoundError

CACHE = "sample_cache"


def section(title: str) -> None:
    print(f"\n{'─' * 50}")
    print(f"  {title}")
    print("─" * 50)


def demo_ping(client: CarbonClient) -> None:
    section("Ping")
    alive = client.ping()
    print(f"Server reachable: {alive}")


def demo_cache_management(client: CarbonClient) -> None:
    section("Cache management")

    client.create_cache(CACHE, eviction="ttl", default_ttl_ms=60_000)
    print(f"Created cache '{CACHE}'")

    caches = client.list_caches()
    names = [c.name for c in caches]
    print(f"All caches: {names}")

    info = client.describe_cache(CACHE)
    print(
        f"Cache info: name={info.name!r}, eviction={info.eviction!r}, ttl_ms={info.default_ttl_ms}"
    )


def demo_data_ops(client: CarbonClient) -> None:
    section("Put / Get / Delete")

    # String key and value
    client.put(CACHE, "greeting", "hello, carbon!")
    value = client.get(CACHE, "greeting")
    print(f"put/get string: {value!r}")

    # Binary value
    client.put(CACHE, b"binary_key", b"\x00\x01\x02\x03")
    raw = client.get(CACHE, b"binary_key")
    print(f"put/get bytes: {(raw)}")

    # Missing key
    missing = client.get(CACHE, "does_not_exist")
    print(f"missing key returns: {missing!r}")

    # Overwrite
    client.put(CACHE, "greeting", "updated value")
    updated = client.get(CACHE, "greeting")
    print(f"after overwrite: {updated!r}")

    # Delete
    deleted = client.delete(CACHE, "greeting")
    print(f"delete existing key: {deleted}")

    gone = client.delete(CACHE, "greeting")
    print(f"delete missing key: {gone}")


def demo_error_handling(client: CarbonClient) -> None:
    section("Error handling")

    try:
        client.drop_cache("__nonexistent__")
    except CacheNotFoundError as e:
        print(f"CacheNotFoundError caught: {e}")


def demo_cleanup(client: CarbonClient) -> None:
    section("Cleanup")
    client.drop_cache(CACHE)
    print(f"Dropped cache '{CACHE}'")


def main() -> None:
    print("Carbon Python Client — Sample App")

    with CarbonClient(
        host="localhost",
        tcp_port=5500,
        http_port=8080,
        username="admin",
        password="admin123",
    ) as client:
        demo_ping(client)
        demo_cache_management(client)
        demo_data_ops(client)
        demo_error_handling(client)
        demo_cleanup(client)

    print("\nDone.")


if __name__ == "__main__":
    main()
