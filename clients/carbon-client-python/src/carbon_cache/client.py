from __future__ import annotations

from types import TracebackType

from ._http import HttpTransport
from ._pool import TcpPool
from .models import CacheInfo


class CarbonClient:
    """
    Unified Carbon cache client.

    Data ops (get/put/delete/ping) use a round-robin pool of TCP connections —
    one per live cluster node, with a background thread that reconciles the
    pool as nodes join or leave.

    Admin ops (create_cache/drop_cache/list_caches) make ad-hoc HTTP calls.

    In standalone mode the pool holds a single connection and no background
    thread is started — behaviour is identical to before.
    """

    def __init__(
        self,
        host: str = "localhost",
        tcp_port: int = 5500,
        http_port: int = 8080,
        username: str = "admin",
        password: str = "admin123",
        *,
        tls: bool = False,
        tcp_timeout: float = 30.0,
        http_timeout: float = 10.0,
        health_interval: float = 10.0,
    ) -> None:
        self._http = HttpTransport(
            host=host,
            port=http_port,
            username=username,
            password=password,
            tls=tls,
            timeout=http_timeout,
        )
        nodes = self._http.cluster_nodes()
        if nodes:
            self._tcp = TcpPool.from_cluster(
                nodes,
                http=self._http,
                health_interval=health_interval,
                seed_host=host,
            )
        else:
            self._tcp = TcpPool.from_single(host, tcp_port)

    # ── Data ops (TCP pool) ──────────────────────────────────────────────────

    def ping(self) -> bool:
        return self._tcp.ping()

    def put(self, cache: str, key: str | bytes, value: str | bytes) -> None:
        self._tcp.put(cache, _to_bytes(key), _to_bytes(value))

    def get(self, cache: str, key: str | bytes) -> bytes | None:
        return self._tcp.get(cache, _to_bytes(key))

    def delete(self, cache: str, key: str | bytes) -> bool:
        return self._tcp.delete(cache, _to_bytes(key))

    # ── Admin ops (HTTP, lazy) ───────────────────────────────────────────────

    def create_cache(
        self,
        name: str,
        eviction: str = "ttl",
        *,
        default_ttl_ms: int | None = None,
        mem_bytes: int | None = None,
        description: str | None = None,
        tags: list[str] | None = None,
    ) -> None:
        self._http.create_cache(
            name,
            eviction,
            default_ttl_ms=default_ttl_ms,
            mem_bytes=mem_bytes,
            description=description,
            tags=tags,
        )

    def drop_cache(self, name: str) -> None:
        self._http.drop_cache(name)

    def list_caches(self) -> list[CacheInfo]:
        return self._http.list_caches()

    def describe_cache(self, name: str) -> CacheInfo:
        return self._http.describe_cache(name)

    # ── Lifecycle ────────────────────────────────────────────────────────────

    def close(self) -> None:
        self._tcp.close()

    def __enter__(self) -> "CarbonClient":
        return self

    def __exit__(
        self,
        exc_type: type[BaseException] | None,
        exc_val: BaseException | None,
        exc_tb: TracebackType | None,
    ) -> None:
        self.close()


def _to_bytes(v: str | bytes) -> bytes:
    return v.encode() if isinstance(v, str) else v
