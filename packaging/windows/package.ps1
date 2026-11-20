param(
    [Parameter(Mandatory = $true)]
    [string]$BinaryPath,
    [Parameter(Mandatory = $true)]
    [string]$Version,
    [Parameter(Mandatory = $true)]
    [string]$OutDir
)

$ErrorActionPreference = "Stop"

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$StagingRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("krono_win_pkg_" + [Guid]::NewGuid().ToString("N"))
$BundleDir = Join-Path $StagingRoot "krono-$Version-windows-x86_64"

try {
    New-Item -ItemType Directory -Force -Path $BundleDir | Out-Null
    Copy-Item -Path $BinaryPath -Destination (Join-Path $BundleDir "krono-desktop.exe") -Force
    foreach ($doc in @("README.md", "LICENSE", "CHANGELOG.md")) {
        $src = Join-Path $RepoRoot $doc
        if (Test-Path $src) {
            Copy-Item -Path $src -Destination (Join-Path $BundleDir $doc) -Force
        }
    }

    New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
    $OutZip = Join-Path (Resolve-Path $OutDir).Path "krono-$Version-windows-x86_64.zip"
    if (Test-Path $OutZip) {
        Remove-Item -Force $OutZip
    }
    Compress-Archive -Path "$BundleDir\*" -DestinationPath $OutZip -CompressionLevel Optimal
    Write-Host "Created $OutZip"
} finally {
    if (Test-Path $StagingRoot) {
        Remove-Item -Recurse -Force $StagingRoot
    }
}
