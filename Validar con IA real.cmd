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

echo Comprobando un flujo breve con tu modelo local de IA...
echo Esta prueba puede tardar varios minutos, pero no envia documentos a Internet.
echo.
if "%~1"=="" (
    "%LIBLEVO_PYTHON%" scripts\validate_real_workflows.py --profile translation
) else (
    "%LIBLEVO_PYTHON%" scripts\validate_real_workflows.py %*
)
set "LIBLEVO_RESULT=%ERRORLEVEL%"
echo.

if "%LIBLEVO_RESULT%"=="0" (
    echo COMPROBACION REAL SUPERADA
) else (
    echo LA COMPROBACION REAL NECESITA ATENCION
)

echo.
pause
exit /b %LIBLEVO_RESULT%
