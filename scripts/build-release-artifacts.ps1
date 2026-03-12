$ErrorActionPreference = "Stop"

$scriptDir = $PSScriptRoot
$repoRoot = (Resolve-Path (Join-Path $scriptDir "..")).Path

$package = "vertexlauncher"
$linuxToolchain = "stable-x86_64-unknown-linux-gnu"
$linuxTarget = "x86_64-unknown-linux-gnu"
$releaseDir = Join-Path $repoRoot "target/release"
$windowsBinary = Join-Path $releaseDir "$package.exe"
$linuxBinary = Join-Path $repoRoot (Join-Path "target/$linuxTarget/release" $package)
$stagedLinuxBinary = Join-Path $releaseDir $package

Push-Location $repoRoot
try {
    Write-Host "Building Windows release binary..."
    & cargo build --release
    if ($LASTEXITCODE -ne 0) {
        throw "cargo build --release failed with exit code $LASTEXITCODE"
    }

    Write-Host "Building Linux GNU release binary..."
    & cargo "+$linuxToolchain" build --release --target $linuxTarget
    if ($LASTEXITCODE -ne 0) {
        throw "cargo +$linuxToolchain build --release --target $linuxTarget failed with exit code $LASTEXITCODE"
    }

    New-Item -ItemType Directory -Force -Path $releaseDir | Out-Null

    if (-not (Test-Path -LiteralPath $windowsBinary -PathType Leaf)) {
        throw "Missing Windows release binary: $windowsBinary"
    }

    if (-not (Test-Path -LiteralPath $linuxBinary -PathType Leaf)) {
        throw "Missing Linux release binary: $linuxBinary"
    }

    Copy-Item -LiteralPath $linuxBinary -Destination $stagedLinuxBinary -Force

    Write-Host ""
    Write-Host "Artifacts ready:"
    Write-Host "  Windows: $windowsBinary"
    Write-Host "  Linux:   $stagedLinuxBinary"
}
finally {
    Pop-Location
}
