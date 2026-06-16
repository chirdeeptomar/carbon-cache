from __future__ import annotations

import itertools
import logging
import threading
from typing import TYPE_CHECKING, Iterator

from ._tcp import TcpTransport
from .exceptions import ConnectionError

if TYPE_CHECKING:
    from ._http import HttpTransport

logger = logging.getLogger(__name__)


class TcpPool:
    """
    Round-robin pool of TcpTransport connections, one per cluster node.

    A background daemon thread periodically fetches /cluster/nodes and
    reconciles the pool: dead nodes are closed and removed; nodes that
    come back alive get a fresh connection added to the rotation.

    In standalone mode (from_single), no background thread is started.
    """

    def __init__(
        self,
        connections: dict[str, TcpTransport],
        http: HttpTransport | None = None,
        health_interval: float = 10.0,
        seed_host: str = "localhost",
    ) -> None:
        self._lock = threading.RLock()
        self._connections: dict[str, TcpTransport] = connections
        self._cycle: Iterator[TcpTransport] = self._make_cycle()
        self._http = http
        self._health_interval = health_interval
        self._seed_host = seed_host
        self._stop = threading.Event()
        self._thread: threading.Thread | None = None

        if http is not None and connections:
            self._thread = threading.Thread(
                target=self._health_loop, daemon=True, name="carbon-pool-health"
            )
            self._thread.start()

    # ── Factory methods ──────────────────────────────────────────────────────

    @classmethod
    def from_single(cls, host: str, port: int) -> "TcpPool":
        addr = f"{host}:{port}"
        transport = TcpTransport(host=host, port=port)
        return cls(connections={addr: transport}, http=None)

    @classmethod
    def from_cluster(
        cls,
        nodes: list[dict],
        http: HttpTransport,
        health_interval: float = 10.0,
        seed_host: str = "localhost",
    ) -> TcpPool:
        connections: dict[str, TcpTransport] = {}
        for n in nodes:
            if not n.get("reachable", False):
                continue
            addr: str = n["tcp_addr"].replace("0.0.0.0", seed_host)
            host, port_str = addr.rsplit(":", 1)
            try:
                connections[addr] = TcpTransport(host=host, port=int(port_str))
            except Exception as e:
                logger.warning("Could not connect to cluster node %s: %s", addr, e)

        if not connections:
            raise ConnectionError("No reachable cluster nodes found during discovery")

        logger.info(
            "Connected to %d cluster node(s): %s", len(connections), list(connections)
        )
        return cls(
            connections=connections,
            http=http,
            health_interval=health_interval,
            seed_host=seed_host,
        )

    # ── Public transport interface (mirrors TcpTransport) ────────────────────

    def ping(self) -> bool:
        return self._dispatch(lambda t: t.ping())

    def put(self, cache: str, key: bytes, value: bytes) -> None:
        self._dispatch(lambda t: t.put(cache, key, value))

    def get(self, cache: str, key: bytes) -> bytes | None:
        return self._dispatch(lambda t: t.get(cache, key))

    def delete(self, cache: str, key: bytes) -> bool:
        return self._dispatch(lambda t: t.delete(cache, key))

    def close(self) -> None:
        self._stop.set()
        with self._lock:
            for t in self._connections.values():
                t.close()
            self._connections.clear()
            self._cycle = iter([])

    # ── Internal ─────────────────────────────────────────────────────────────

    def _make_cycle(self) -> Iterator[TcpTransport]:
        items = list(self._connections.values())
        return itertools.cycle(items) if items else iter([])

    def _next(self) -> TcpTransport:
        with self._lock:
            if not self._connections:
                raise ConnectionError("No live cluster nodes available")
            return next(self._cycle)

    def _remove(self, addr: str) -> None:
        with self._lock:
            t = self._connections.pop(addr, None)
            if t is not None:
                try:
                    t.close()
                except Exception:
                    pass
                self._cycle = self._make_cycle()
                logger.warning(
                    "Removed dead node %s from pool (%d remaining)",
                    addr,
                    len(self._connections),
                )

    def _dispatch(self, op):
        with self._lock:
            attempts = max(len(self._connections), 1)

        last_exc: Exception = ConnectionError("No live cluster nodes available")
        tried: set[str] = set()

        for _ in range(attempts):
            transport = self._next()
            # Find its addr for removal on failure
            with self._lock:
                addr = next(
                    (a for a, t in self._connections.items() if t is transport),
                )
            if addr in tried:
                continue
            tried.add(addr)
            try:
                return op(transport)
            except ConnectionError as e:
                last_exc = e
                if addr:
                    self._remove(addr)

        raise last_exc

    def _reconcile(self, nodes: list[dict]) -> None:
        with self._lock:
            for n in nodes:
                raw_addr: str = n.get("tcp_addr", "")
                addr = raw_addr.replace("0.0.0.0", self._seed_host)
                reachable: bool = n.get("reachable", False)

                if reachable and addr not in self._connections:
                    host, port_str = addr.rsplit(":", 1)
                    try:
                        t = TcpTransport(host=host, port=int(port_str))
                        self._connections[addr] = t
                        logger.info("Node %s rejoined cluster — added to pool", addr)
                    except Exception as e:
                        logger.debug("Node %s still unreachable: %s", addr, e)
                elif not reachable and addr in self._connections:
                    self._connections[addr].close()
                    del self._connections[addr]
                    logger.warning("Node %s marked dead — removed from pool", addr)

            self._cycle = self._make_cycle()

    def _health_loop(self) -> None:
        while not self._stop.is_set():
            self._stop.wait(self._health_interval)
            if self._stop.is_set():
                break
            try:
                assert self._http is not None
                nodes = self._http.cluster_nodes()
                if nodes:
                    self._reconcile(nodes)
            except Exception as e:
                logger.debug("Health check error (ignored): %s", e)
