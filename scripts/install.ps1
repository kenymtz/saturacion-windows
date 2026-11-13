# Compila la app en modo release, la instala en
# %LOCALAPPDATA%\Programs\Ajustes de pantalla y crea un acceso directo en el escritorio.
# Volver a correrlo actualiza la instalación (cerrar la app antes, con "Salir").
# Uso, desde la raíz del repo:
#   powershell -ExecutionPolicy Bypass -File scripts/install.ps1
$ErrorActionPreference = 'Stop'

$repo = Split-Path -Parent $PSScriptRoot
$cargo = Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe'
if (-not (Test-Path $cargo)) { $cargo = 'cargo' }

& $cargo build --release --manifest-path (Join-Path $repo 'Cargo.toml')
if ($LASTEXITCODE -ne 0) { throw "cargo build falló (código $LASTEXITCODE)" }

$dest = Join-Path $env:LOCALAPPDATA 'Programs\Ajustes de pantalla'
New-Item -ItemType Directory -Force $dest | Out-Null

$exe = Join-Path $dest 'Ajustes de pantalla.exe'
if (Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $exe }) {
    throw "La app instalada está abierta. Cerrala con 'Salir' desde la bandeja y volvé a correr el script."
}
Copy-Item (Join-Path $repo 'target\release\saturacion.exe') $exe -Force

# Ícono .ico con el PNG de la app embebido (formato admitido desde Windows Vista).
$pngBytes = [System.IO.File]::ReadAllBytes((Join-Path $repo 'ui\icon.png'))
$ico = Join-Path $dest 'icon.ico'
$stream = New-Object System.IO.MemoryStream
$writer = New-Object System.IO.BinaryWriter $stream
$writer.Write([uint16]0)                # reservado
$writer.Write([uint16]1)                # tipo: ícono
$writer.Write([uint16]1)                # cantidad de imágenes
$writer.Write([byte]64)                 # ancho
$writer.Write([byte]64)                 # alto
$writer.Write([byte]0)                  # paleta
$writer.Write([byte]0)                  # reservado
$writer.Write([uint16]1)                # planos
$writer.Write([uint16]32)               # bits por pixel
$writer.Write([uint32]$pngBytes.Length) # tamaño de la imagen
$writer.Write([uint32]22)               # posición de la imagen
$writer.Write($pngBytes)
$writer.Flush()
[System.IO.File]::WriteAllBytes($ico, $stream.ToArray())
$writer.Dispose()

$desktop = [Environment]::GetFolderPath('Desktop')
$link = Join-Path $desktop 'Ajustes de pantalla.lnk'
$shell = New-Object -ComObject WScript.Shell
$shortcut = $shell.CreateShortcut($link)
$shortcut.TargetPath = $exe
$shortcut.WorkingDirectory = $dest
$shortcut.IconLocation = "$ico,0"
$shortcut.Description = 'Saturación, contraste y brillo de la pantalla'
$shortcut.Save()

Write-Output "Instalado en: $exe"
Write-Output "Acceso directo: $link"
