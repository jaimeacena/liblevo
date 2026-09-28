param(
    [switch]$InstallInnoSetup,
    [switch]$PreflightOnly,
    [string]$PythonPath
)

$ErrorActionPreference = "Stop"
$repository = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
Set-Location $repository
$venvPython = Join-Path $repository ".venv\Scripts\python.exe"
$python = if ($PythonPath) {
    (Resolve-Path -LiteralPath $PythonPath -ErrorAction Stop).Path
}
elseif (Test-Path -LiteralPath $venvPython) {
    $venvPython
}
else {
    (Get-Command python -ErrorAction Stop).Source
}
if (-not (Test-Path -LiteralPath $python -PathType Leaf)) {
    throw "No se encontró el ejecutable Python indicado."
}
& $python "distribution\windows\check_runtime.py"
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}
if ($PreflightOnly) {
    Write-Output "El entorno local permite construir el paquete con IA integrada."
    exit 0
}

if ($InstallInnoSetup) {
    $installerDefinition = Get-Content -LiteralPath "distribution\windows\Liblevo.iss" -Raw
    $versionMatch = [regex]::Match($installerDefinition, '#define AppVersion "([^"]+)"')
    if (-not $versionMatch.Success) {
        throw "No se pudo determinar la versión del instalador."
    }
    $installerPath = Join-Path $repository (
        "outputs\Liblevo-Setup-{0}.exe" -f $versionMatch.Groups[1].Value
    )
    if (Test-Path -LiteralPath $installerPath) {
        throw "Ya existe un instalador de esta versión. Consérvalo y usa otra versión."
    }
}

$packageDirectory = Join-Path $repository "outputs\package"
$buildCacheParent = Join-Path $repository "outputs\build-cache"
foreach ($parentPath in @((Join-Path $repository "outputs"), $packageDirectory, $buildCacheParent)) {
    if (Test-Path -LiteralPath $parentPath) {
        $parentItem = Get-Item -LiteralPath $parentPath
        if (($parentItem.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
            throw "La carpeta de salida es un enlace; no se moverán paquetes a través de él."
        }
    }
}
$buildId = [guid]::NewGuid().ToString("N")
$candidateDirectory = Join-Path $packageDirectory ("candidate-" + $buildId)
$buildCache = Join-Path $buildCacheParent ("pyinstaller-" + $buildId)
if (Test-Path -LiteralPath $candidateDirectory) {
    throw "Ya existe la carpeta de construcción candidata."
}
New-Item -ItemType Directory -Force -Path $candidateDirectory, $buildCache | Out-Null
$noticesPath = Join-Path $candidateDirectory "THIRD-PARTY-NOTICES.txt"
& $python "distribution\windows\generate_notices.py" `
    --output $noticesPath
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}
$previousNoticesPath = $env:LIBLEVO_NOTICES_PATH
$previousPyInstallerConfig = $env:PYINSTALLER_CONFIG_DIR
try {
    $env:LIBLEVO_NOTICES_PATH = $noticesPath
    $env:PYINSTALLER_CONFIG_DIR = Join-Path $buildCache "config"
    & $python -m PyInstaller --noconfirm --clean `
        --workpath $buildCache `
        --distpath $candidateDirectory `
        "distribution\windows\Liblevo.spec"
    if ($LASTEXITCODE -ne 0) {
        throw "No se pudo construir el candidato; el paquete anterior permanece intacto."
    }
}
finally {
    $env:LIBLEVO_NOTICES_PATH = $previousNoticesPath
    $env:PYINSTALLER_CONFIG_DIR = $previousPyInstallerConfig
}

$candidateBundle = Join-Path $candidateDirectory "Liblevo"
$applicationPath = Join-Path $candidateBundle "Liblevo.exe"
if (-not (Test-Path -LiteralPath $applicationPath -PathType Leaf)) {
    throw "La construcción no produjo el ejecutable esperado."
}
$candidateItem = Get-Item -LiteralPath $candidateBundle
if (($candidateItem.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
    throw "El candidato es un enlace; no se moverá automáticamente."
}
$resolvedCandidateParent = [System.IO.Path]::GetDirectoryName(
    (Resolve-Path -LiteralPath $candidateBundle).Path
)
if (-not $resolvedCandidateParent.Equals(
    (Resolve-Path -LiteralPath $candidateDirectory).Path,
    [System.StringComparison]::OrdinalIgnoreCase
)) {
    throw "El candidato resuelto queda fuera de su carpeta de construcción."
}
$previousQtPlatform = $env:QT_QPA_PLATFORM
try {
    $env:QT_QPA_PLATFORM = "offscreen"
    $smokeProcess = Start-Process `
        -FilePath $applicationPath `
        -ArgumentList "--package-smoke" `
        -Wait `
        -PassThru `
        -WindowStyle Hidden
}
finally {
    $env:QT_QPA_PLATFORM = $previousQtPlatform
}
if ($smokeProcess.ExitCode -ne 0) {
    throw "El candidato no superó la prueba de arranque; el paquete anterior permanece intacto."
}

$bundleDirectory = Join-Path $packageDirectory "Liblevo"
$previousBundle = Join-Path $packageDirectory ("Liblevo.previous-" + $buildId)
$oldBundleMoved = $false
if (Test-Path -LiteralPath $bundleDirectory) {
    $bundleItem = Get-Item -LiteralPath $bundleDirectory
    if (($bundleItem.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
        throw "La carpeta de paquete anterior es un enlace; no se moverá automáticamente."
    }
    $resolvedBundle = (Resolve-Path -LiteralPath $bundleDirectory).Path
    $resolvedParent = [System.IO.Path]::GetDirectoryName($resolvedBundle)
    if (-not $resolvedParent.Equals(
        (Resolve-Path -LiteralPath $packageDirectory).Path,
        [System.StringComparison]::OrdinalIgnoreCase
    )) {
        throw "La carpeta de paquete resuelta queda fuera de outputs\package."
    }
    Move-Item -LiteralPath $bundleDirectory -Destination $previousBundle
    $oldBundleMoved = $true
}
try {
    Move-Item -LiteralPath $candidateBundle -Destination $bundleDirectory
}
catch {
    if ($oldBundleMoved -and -not (Test-Path -LiteralPath $bundleDirectory)) {
        Move-Item -LiteralPath $previousBundle -Destination $bundleDirectory
    }
    throw
}
Write-Output "Paquete local probado: $bundleDirectory"
if ($oldBundleMoved) {
    Write-Output "Paquete anterior recuperable: $previousBundle"
}

function Find-InnoSetupCompiler {
    $command = Get-Command iscc.exe -ErrorAction SilentlyContinue
    if ($command) {
        return $command.Source
    }
    $compilerCandidates = @(
        (Join-Path $env:LOCALAPPDATA "Programs\Inno Setup 6\ISCC.exe"),
        "C:\Program Files (x86)\Inno Setup 6\ISCC.exe",
        "C:\Program Files\Inno Setup 6\ISCC.exe"
    )
    foreach ($candidate in $compilerCandidates) {
        if (Test-Path -LiteralPath $candidate) {
            return $candidate
        }
    }
    return $null
}

if ($InstallInnoSetup) {
    $compilerPath = Find-InnoSetupCompiler
    if (-not $compilerPath) {
        throw "Inno Setup 6 no está disponible. Instálalo aparte si necesitas crear un instalador."
    }
    & $compilerPath "distribution\windows\Liblevo.iss"
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
}

function Remove-OwnBuildDirectory {
    param([string]$Path, [string]$Parent)

    $item = Get-Item -LiteralPath $Path
    if (($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
        throw "Una carpeta temporal es un enlace; no se eliminará automáticamente."
    }
    $resolved = (Resolve-Path -LiteralPath $Path).Path
    $resolvedParent = (Resolve-Path -LiteralPath $Parent).Path
    if (-not [System.IO.Path]::GetDirectoryName($resolved).Equals(
        $resolvedParent,
        [System.StringComparison]::OrdinalIgnoreCase
    )) {
        throw "Una carpeta temporal queda fuera de su destino; no se eliminará automáticamente."
    }
    Remove-Item -LiteralPath $resolved -Recurse -Force
}

Remove-OwnBuildDirectory -Path $buildCache -Parent $buildCacheParent
Remove-OwnBuildDirectory -Path $candidateDirectory -Parent $packageDirectory
