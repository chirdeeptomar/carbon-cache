from __future__ import annotations
from dataclasses import dataclass, field


@dataclass
class NodeInfo:
    id: int
    tcp_addr: str
    http_addr: str
    is_leader: bool
    reachable: bool


@dataclass
class CacheInfo:
    name: str
    eviction: str
    description: str | None = None
    default_ttl_ms: int | None = None
    tags: list[str] = field(default_factory=list)


@dataclass
class LoginResponse:
    token: str
    expires_in: int
    username: str
