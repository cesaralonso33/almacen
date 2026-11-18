param(
    [Parameter(Mandatory = $true)]
    [string]$Version,
    [string]$Notas = "",
    [string]$BaseUrl = "https://129-121-117-177.sslip.io",
    [string]$Destino = "infra/updates"
)

$ErrorActionPreference = "Stop"
$raiz = Split-Path -Parent $PSScriptRoot
Set-Location $raiz

$configuracion = Get-Content "desktop/src-tauri/tauri.conf.json" -Raw | ConvertFrom-Json
if ($configuracion.version -ne $Version) {
    throw "La versión de tauri.conf.json es $($configuracion.version), no $Version."
}

$paquete = "desktop/src-tauri/target/release/bundle/nsis/Custodia CradSystem_${Version}_x64-setup.exe"
$firma = "$paquete.sig"
if (-not (Test-Path $paquete)) {
    throw "No existe el instalador: $paquete. Ejecute npm run tauri build con TAURI_SIGNING_PRIVATE_KEY."
}
if (-not (Test-Path $firma)) {
    throw "No existe la firma: $firma. Configure TAURI_SIGNING_PRIVATE_KEY y vuelva a compilar."
}

New-Item -ItemType Directory -Force -Path $Destino | Out-Null
$nombre = Split-Path $paquete -Leaf
Copy-Item $paquete (Join-Path $Destino $nombre) -Force
Copy-Item $firma (Join-Path $Destino "$nombre.sig") -Force

$contenidoFirma = (Get-Content $firma -Raw).Trim()
$nombreUrl = [uri]::EscapeDataString($nombre)
$manifiesto = [ordered]@{
    version   = $Version
    notes     = $Notas
    pub_date  = (Get-Date).ToUniversalTime().ToString("o")
    platforms = [ordered]@{
        "windows-x86_64" = [ordered]@{
            signature = $contenidoFirma
            url       = "$($BaseUrl.TrimEnd('/'))/updates/$nombreUrl"
        }
    }
}

$rutaManifiesto = Join-Path $Destino "latest.json"
$json = $manifiesto | ConvertTo-Json -Depth 6
[IO.File]::WriteAllText((Join-Path $raiz $rutaManifiesto), $json, (New-Object Text.UTF8Encoding($false)))

Write-Host "Publicado $nombre"
Write-Host "Manifiesto: $rutaManifiesto"
Write-Host "Copie $Destino al VPS sobre el volumen /updates de la API."
