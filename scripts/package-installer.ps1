param(
    [Parameter(Mandatory = $true)]
    [string]$StubExe,

    [Parameter(Mandatory = $true)]
    [string]$AppExe,

    [Parameter(Mandatory = $true)]
    [string]$Version,

    [Parameter(Mandatory = $true)]
    [string]$OutExe
)

$ErrorActionPreference = 'Stop'

if (-not (Test-Path -LiteralPath $StubExe)) {
    throw "Stub installer executable not found: $StubExe"
}
if (-not (Test-Path -LiteralPath $AppExe)) {
    throw "Launcher executable not found: $AppExe"
}

Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem

$tempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("mse-pack-" + [Guid]::NewGuid().ToString('N'))
$stageDir = Join-Path $tempRoot 'stage'
$zipPath = Join-Path $tempRoot 'payload.zip'

try {
    New-Item -ItemType Directory -Path $stageDir | Out-Null
    Copy-Item -LiteralPath $AppExe -Destination (Join-Path $stageDir 'MSE Launcher.exe') -Force

    $manifestObj = [ordered]@{
        version    = $Version
        appExeName = 'MSE Launcher.exe'
    }
    $manifestJson = $manifestObj | ConvertTo-Json -Compress
    [System.IO.File]::WriteAllText(
        (Join-Path $stageDir 'manifest.json'),
        $manifestJson,
        [System.Text.UTF8Encoding]::new($false)
    )

    [System.IO.Compression.ZipFile]::CreateFromDirectory(
        $stageDir,
        $zipPath,
        [System.IO.Compression.CompressionLevel]::Optimal,
        $false
    )

    $outDir = Split-Path -Parent $OutExe
    if ($outDir -and -not (Test-Path -LiteralPath $outDir)) {
        New-Item -ItemType Directory -Path $outDir | Out-Null
    }

    $stubLen = [uint64](Get-Item -LiteralPath $StubExe).Length
    $outStream = [System.IO.File]::Create($OutExe)
    try {
        $stubStream = [System.IO.File]::OpenRead($StubExe)
        try {
            $stubStream.CopyTo($outStream)
        } finally {
            $stubStream.Dispose()
        }

        $zipStream = [System.IO.File]::OpenRead($zipPath)
        try {
            $zipStream.CopyTo($outStream)
        } finally {
            $zipStream.Dispose()
        }

        $offsetBytes = [System.BitConverter]::GetBytes($stubLen)
        if (-not [System.BitConverter]::IsLittleEndian) {
            [System.Array]::Reverse($offsetBytes)
        }
        $magicBytes = [System.Text.Encoding]::ASCII.GetBytes('MSEINST1')
        $outStream.Write($offsetBytes, 0, 8)
        $outStream.Write($magicBytes, 0, 8)
    } finally {
        $outStream.Dispose()
    }

    Write-Host "Packaged custom installer: $OutExe"
} finally {
    if (Test-Path -LiteralPath $tempRoot) {
        Remove-Item -LiteralPath $tempRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}
