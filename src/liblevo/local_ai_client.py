"""A worker-owned asynchronous HTTP client with a synchronous calling boundary."""

from __future__ import annotations

import asyncio
from collections.abc import Coroutine
from contextvars import copy_context
from types import TracebackType
from typing import Any, Self

import httpx


class LocalAiClient:
    """Reuse one event loop and connection pool throughout a local processing phase."""

    def __init__(
        self,
        *,
        timeout: float | httpx.Timeout = 120.0,
        transport: httpx.BaseTransport | httpx.AsyncBaseTransport | None = None,
    ) -> None:
        if transport is not None and not isinstance(transport, httpx.AsyncBaseTransport):
            raise TypeError("El transporte local debe admitir solicitudes asíncronas.")
        self._runner = asyncio.Runner()
        self.http = httpx.AsyncClient(
            timeout=timeout, transport=transport, trust_env=False, follow_redirects=False
        )

    @property
    def timeout(self) -> httpx.Timeout:
        return self.http.timeout

    def run[T](self, request: Coroutine[Any, Any, T]) -> T:
        # Each invocation inherits the current document's private metrics context.
        return self._runner.run(request, context=copy_context())

    def __enter__(self) -> Self:
        return self

    def __exit__(
        self,
        exc_type: type[BaseException] | None,
        exc_value: BaseException | None,
        traceback: TracebackType | None,
    ) -> None:
        self.close()

    def close(self) -> None:
        if not self.http.is_closed:
            try:
                self.run(self.http.aclose())
            finally:
                self._runner.close()
