# Compila la app en release y genera el instalador con Inno Setup en dist/.
# Requiere Inno Setup 6: winget install --id JRSoftware.InnoSetup -e
# Uso, desde la raíz del repo:
#   powershell -ExecutionPolicy Bypass -File scripts/build-installer.ps1
$ErrorActionPreference = 'Stop'

$repo = Split-Path -Parent $PSScriptRoot
$cargo = Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe'
if (-not (Test-Path $cargo)) { $cargo = 'cargo' }

& $cargo build --release --manifest-path (Join-Path $repo 'Cargo.toml')
if ($LASTEXITCODE -ne 0) { throw "cargo build falló (código $LASTEXITCODE)" }

$versionLine = Select-String -Path (Join-Path $repo 'Cargo.toml') -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1
$version = $versionLine.Matches[0].Groups[1].Value

$iscc = @(
    (Join-Path $env:LOCALAPPDATA 'Programs\Inno Setup 6\ISCC.exe'),
    (Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 6\ISCC.exe'),
    (Join-Path $env:ProgramFiles 'Inno Setup 6\ISCC.exe')
) | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $iscc) { throw 'No se encontró Inno Setup 6. Instalalo con: winget install --id JRSoftware.InnoSetup -e' }

& $iscc "/DAppVersion=$version" (Join-Path $repo 'installer\setup.iss')
if ($LASTEXITCODE -ne 0) { throw "ISCC falló (código $LASTEXITCODE)" }

Write-Output "Instalador: $(Join-Path $repo "dist\AjustesDePantalla-Setup-$version.exe")"
