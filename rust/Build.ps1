param([switch]$Package)
$ErrorActionPreference = 'Stop'
Set-Location -LiteralPath $PSScriptRoot
$cargo = Get-Command cargo -ErrorAction SilentlyContinue
if (-not $cargo) {
    $cargoPath = Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe'
    if (-not (Test-Path -LiteralPath $cargoPath)) { throw 'Install Rust from https://rustup.rs and C++ Build Tools with Windows SDK.' }
} else { $cargoPath = $cargo.Source }
& $cargoPath fmt --check
if ($LASTEXITCODE) { throw 'Formatting failed' }
& $cargoPath test --locked
if ($LASTEXITCODE) { throw 'Tests failed' }
& $cargoPath clippy --locked --all-targets -- -D warnings
if ($LASTEXITCODE) { throw 'Clippy failed' }
& $cargoPath build --locked --release
if ($LASTEXITCODE) { throw 'Build failed' }
& (Join-Path $PSScriptRoot 'tools\Set-ExecutableIcon.ps1') -InputExe (Join-Path $PSScriptRoot 'target\release\claude-mascot.exe') -IconPath (Join-Path $PSScriptRoot 'assets\claude-icon.ico')
if ($Package) {
    $packageDir = Join-Path $PSScriptRoot 'dist'
    New-Item -ItemType Directory -Force -Path $packageDir | Out-Null
    Copy-Item -LiteralPath 'target\release\claude-mascot.exe','README.md','Install.ps1','Uninstall.ps1' -Destination $packageDir -Force
    $docsDir = Join-Path $packageDir 'docs'
    New-Item -ItemType Directory -Force -Path $docsDir | Out-Null
    Get-ChildItem -LiteralPath (Join-Path $PSScriptRoot 'docs') -File | Copy-Item -Destination $docsDir -Force
    Compress-Archive -LiteralPath (Join-Path $packageDir 'claude-mascot.exe'),(Join-Path $packageDir 'README.md'),(Join-Path $packageDir 'Install.ps1'),(Join-Path $packageDir 'Uninstall.ps1'),$docsDir -DestinationPath 'claude-mascot-windows-x64.zip' -Force
}
