param(
    [string]$ExecutablePath = (Join-Path $PSScriptRoot 'target\release\claude-mascot.exe'),
    [string]$IconPath = (Join-Path $PSScriptRoot 'assets\claude-icon.ico'),
    [string]$OutputDirectory = (Join-Path $PSScriptRoot 'dist'),
    [string]$Version
)

$ErrorActionPreference = 'Stop'
Set-Location -LiteralPath $PSScriptRoot

$cargoToml = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'Cargo.toml') -Raw
if (-not $Version) {
    $versionMatch = [regex]::Match($cargoToml, '(?m)^version\s*=\s*"(?<version>\d+\.\d+\.\d+)"\s*$')
    if (-not $versionMatch.Success) { throw 'Could not read a three-part package version from Cargo.toml.' }
    $Version = $versionMatch.Groups['version'].Value
}
if ($Version -notmatch '^\d+\.\d+\.\d+$') {
    throw "MSI version must be three numeric components (major.minor.patch); received '$Version'."
}

$ExecutablePath = [IO.Path]::GetFullPath($ExecutablePath)
if (-not (Test-Path -LiteralPath $ExecutablePath -PathType Leaf)) {
    throw "Release executable not found: $ExecutablePath. Build it first with .\Build.ps1."
}
$IconPath = [IO.Path]::GetFullPath($IconPath)
if (-not (Test-Path -LiteralPath $IconPath -PathType Leaf)) {
    throw "Application icon not found: $IconPath"
}

$wixCommand = Get-Command wix -ErrorAction SilentlyContinue
if (-not $wixCommand) {
    $globalWix = Join-Path $env:USERPROFILE '.dotnet\tools\wix.exe'
    if (Test-Path -LiteralPath $globalWix) { $wixCommand = Get-Item -LiteralPath $globalWix }
}
if (-not $wixCommand) {
    throw 'WiX Toolset 6.0.2 is required. Install it for the current user with: dotnet tool install --global wix --version 6.0.2'
}
$wixVersion = (& $wixCommand.Source --version | Out-String).Trim()
if ($wixVersion -notmatch '^6\.0\.2\+') {
    throw "This script is authored for WiX Toolset 6.0.2; found '$wixVersion'."
}

$installerDirectory = Join-Path $PSScriptRoot 'installer'
$markerPath = Join-Path $installerDirectory 'install-mode-msi.marker'
if (-not (Test-Path -LiteralPath $markerPath -PathType Leaf)) {
    throw "The MSI ownership marker source is missing: $markerPath"
}
New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
$OutputDirectory = [IO.Path]::GetFullPath($OutputDirectory)
$msiPath = Join-Path $OutputDirectory "claude-mascot-$Version-x64.msi"
$wxsPath = Join-Path $installerDirectory 'Package.wxs'

& $wixCommand.Source build $wxsPath -arch x64 -define "ProductVersion=$Version" -define "ExecutablePath=$ExecutablePath" -define "MarkerPath=$markerPath" -define "IconPath=$IconPath" -out $msiPath
if ($LASTEXITCODE -ne 0) { throw "WiX build failed with exit code $LASTEXITCODE." }

Write-Host "Created $msiPath"
