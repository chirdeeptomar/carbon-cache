import socket
import pytest


def _is_port_open(host: str, port: int) -> bool:
    try:
        s = socket.create_connection((host, port), timeout=1)
        s.close()
        return True
    except OSError:
        return False


def pytest_configure(config):
    config.addinivalue_line(
        "markers", "integration: mark test as requiring a running Carbon server"
    )


@pytest.fixture(scope="session", autouse=True)
def require_carbon():
    tcp_up = _is_port_open("localhost", 5500)
    http_up = _is_port_open("localhost", 8080)
    if not tcp_up or not http_up:
        pytest.skip(
            "Carbon server not reachable on localhost:5500 (TCP) and localhost:8080 (HTTP). "
            "Start Carbon first: cargo run -p carbon-server"
        )
