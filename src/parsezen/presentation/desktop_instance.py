"""One desktop writer per local profile, with activation of the existing window."""

from __future__ import annotations

import hashlib
from collections.abc import Callable
from pathlib import Path

from PySide6.QtCore import QLockFile
from PySide6.QtNetwork import QLocalServer, QLocalSocket


class DesktopInstance:
    """Hold the process lock until all window persistence has finished."""

    def __init__(self, profile: Path) -> None:
        profile.mkdir(parents=True, exist_ok=True)
        identity = str(profile.resolve()).casefold().encode("utf-8")
        self._name = "parsezen-" + hashlib.sha256(identity).hexdigest()[:32]
        self._lock = QLockFile(str(profile / "desktop.lock"))
        # A long conversion must never make a live owner's lock look stale.
        # QLockFile still reclaims a lock whose owning process no longer exists.
        self._lock.setStaleLockTime(0)
        self._server = QLocalServer()
        self._server.setSocketOptions(QLocalServer.SocketOption.UserAccessOption)
        self._server.newConnection.connect(self._activate)
        self._callback: Callable[[], None] | None = None
        self._pending_activation = False

    def acquire(self) -> bool:
        if not self._lock.tryLock(0):
            if self._lock.error() != QLockFile.LockError.LockFailedError:
                raise OSError("No se pudo proteger la cola de Parsezen.")
            socket = QLocalSocket()
            socket.connectToServer(self._name)
            socket.waitForConnected(500)
            socket.close()
            return False
        QLocalServer.removeServer(self._name)
        if not self._server.listen(self._name):
            self.close()
            raise OSError("No se pudo abrir la sesión de Parsezen de forma segura.")
        return True

    def on_activation(self, callback: Callable[[], None]) -> None:
        self._callback = callback
        if self._pending_activation:
            self._pending_activation = False
            callback()

    def _activate(self) -> None:
        while self._server.hasPendingConnections():
            socket = self._server.nextPendingConnection()
            if socket is not None:
                socket.close()
                socket.deleteLater()
        if self._callback is None:
            self._pending_activation = True
        else:
            self._callback()

    def close(self) -> None:
        self._server.close()
        self._lock.unlock()
