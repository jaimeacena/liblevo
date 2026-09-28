@echo off
setlocal
cd /d "%~dp0"

set "LIBLEVO_PYTHON=%~dp0.venv\Scripts\python.exe"
if not exist "%LIBLEVO_PYTHON%" (
    echo Liblevo todavia no esta instalado en esta carpeta.
    echo Pide a Codex que prepare el entorno de desarrollo.
    pause
    exit /b 1
)

echo Comprobando las funciones basicas de Liblevo...
echo.
"%LIBLEVO_PYTHON%" -m pytest -m acceptance
set "LIBLEVO_RESULT=%ERRORLEVEL%"
echo.

if "%LIBLEVO_RESULT%"=="0" (
    echo COMPROBACION SUPERADA
) else (
    echo LA COMPROBACION NECESITA ATENCION
)

echo.
pause
exit /b %LIBLEVO_RESULT%
