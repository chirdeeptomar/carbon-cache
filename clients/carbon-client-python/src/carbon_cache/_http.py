from __future__ import annotations

import logging

import httpx

from .exceptions import AuthError, CacheNotFoundError, ServerError
from .models import CacheInfo


class HttpTransport:
    """
    Ad-hoc HTTP transport for admin operations.

    No persistent connection is held — each call opens and closes an httpx.Client.
    Authentication uses HTTP Basic Auth on every request.
    """

    def __init__(
        self,
        host: str = "localhost",
        port: int = 8080,
        username: str = "admin",
        password: str = "admin123",
        *,
        tls: bool = False,
        timeout: float = 10.0,
    ) -> None:
        scheme = "https" if tls else "http"
        self._base_url = f"{scheme}://{host}:{port}"
        self._auth = (username, password)
        self._timeout = timeout
        self.logger = logging.getLogger(__name__)

    def _request(self, method: str, path: str, **kwargs) -> httpx.Response:
        with httpx.Client(
            base_url=self._base_url,
            auth=self._auth,
            timeout=self._timeout,
        ) as client:
            resp = client.request(method, path, **kwargs)
        if resp.status_code == 401:
            raise AuthError("Authentication failed — check username and password")
        return resp

    def health(self) -> str:
        resp = self._request("GET", "/health")
        return resp.json().get("message", "")

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
        body: dict = {"name": name, "eviction": eviction, "policy": ""}
        if default_ttl_ms is not None:
            body["default_ttl_ms"] = default_ttl_ms
        if mem_bytes is not None:
            body["mem_bytes"] = mem_bytes
        if description is not None:
            body["description"] = description
        if tags is not None:
            body["tags"] = tags
        resp = self._request("POST", "/admin/caches", json=body)
        if resp.status_code == 400:
            raise ValueError(resp.json().get("error", "Validation error"))
        if not resp.is_success:
            raise ServerError(f"create_cache failed: {resp.status_code} {resp.text}")

    def drop_cache(self, name: str) -> None:
        resp = self._request("DELETE", f"/admin/caches/{name}")
        if resp.status_code == 404:
            raise CacheNotFoundError(name)
        if not resp.is_success:
            raise ServerError(f"drop_cache failed: {resp.status_code} {resp.text}")

    def list_caches(self) -> list[CacheInfo]:
        resp = self._request("GET", "/admin/caches")
        if not resp.is_success:
            raise ServerError(f"list_caches failed: {resp.status_code}")
        data = resp.json()
        self.logger.debug(f"list_caches response: {data}")
        caches = data if isinstance(data, list) else data.get("caches", [])

        return [
            CacheInfo(
                name=c["name"],
                eviction=c.get("eviction", ""),
                description=c.get("description"),
                default_ttl_ms=c.get("default_ttl_ms"),
                tags=c.get("tags") or [],
            )
            for c in caches
        ]

    def cluster_nodes(self) -> list[dict]:
        try:
            resp = self._request("GET", "/cluster/nodes")
            if resp.status_code != 200:
                return []
            data: dict = resp.json()
            if data.get("mode") != "cluster":
                return []
            return data.get("nodes", [])
        except Exception:
            return []

    def describe_cache(self, name: str) -> CacheInfo:
        resp = self._request("GET", f"/admin/caches/{name}")
        if resp.status_code == 404:
            raise CacheNotFoundError(name)
        if not resp.is_success:
            raise ServerError(f"describe_cache failed: {resp.status_code}")
        c = resp.json().get("info")
        print(f"describe_cache response: {c}")
        return CacheInfo(
            name=c["name"],
            eviction=c.get("eviction", ""),
            description=c.get("description"),
            default_ttl_ms=c.get("default_ttl_ms"),
            tags=c.get("tags") or [],
        )
