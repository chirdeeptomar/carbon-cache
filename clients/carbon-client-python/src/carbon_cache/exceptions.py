class CarbonError(Exception):
    """Base exception for all Carbon client errors."""


class ConnectionError(CarbonError):
    """TCP connection to Carbon server failed or was lost."""


class KeyNotFoundError(CarbonError):
    """Key does not exist in the cache."""


class CacheNotFoundError(CarbonError):
    """Named cache does not exist on the server."""


class AuthError(CarbonError):
    """Authentication or authorisation failure."""


class ServerError(CarbonError):
    """Server returned an ERROR response."""
