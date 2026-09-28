@echo off
setlocal
cd /d "%~dp0"

set "LIBLEVO_PYTHON=%~dp0.venv\Scripts\pythonw.exe"
if not exist "%LIBLEVO_PYTHON%" (
    echo Liblevo todavia no esta instalado en esta carpeta.
    echo Pide a Codex que prepare el entorno de desarrollo.
    pause
    exit /b 1
)

rem Always import Liblevo from this checkout, even if the virtual environment
rem still contains an older non-editable installation.
set "PYTHONPATH=%~dp0src;%PYTHONPATH%"
start "" "%LIBLEVO_PYTHON%" -m liblevo
if errorlevel 1 (
    echo Windows no pudo abrir Liblevo.
    pause
    exit /b 1
)

endlocal
