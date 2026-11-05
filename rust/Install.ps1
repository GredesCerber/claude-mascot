param([switch]$Autostart)
$ErrorActionPreference = 'Stop'
$destination = [IO.Path]::GetFullPath((Join-Path $env:LOCALAPPDATA 'claude-mascot-rust'))
$marker = Join-Path $destination 'install-mode-msi.marker'
if (Test-Path -LiteralPath $marker -PathType Leaf) {
    throw 'This directory is owned by the MSI installation. Use Windows Settings or Apps & Features to manage it, or extract the portable package to a separate location.'
}

$source = Join-Path $PSScriptRoot 'claude-mascot.exe'
if (-not (Test-Path -LiteralPath $source -PathType Leaf)) { $source = Join-Path $PSScriptRoot 'target\release\claude-mascot.exe' }
if (-not (Test-Path -LiteralPath $source -PathType Leaf)) { throw 'Build with .\Build.ps1 or extract the portable release first.' }
$source = [IO.Path]::GetFullPath($source)
$sourceUninstaller = Join-Path $PSScriptRoot 'Uninstall.ps1'
if (-not (Test-Path -LiteralPath $sourceUninstaller -PathType Leaf)) { throw "Portable uninstaller not found: $sourceUninstaller" }
$sourceUninstaller = [IO.Path]::GetFullPath($sourceUninstaller)

$exe = Join-Path $destination 'claude-mascot.exe'
$uninstaller = Join-Path $destination 'Uninstall.ps1'
$programs = [Environment]::GetFolderPath('Programs')
$desktop = [Environment]::GetFolderPath('Desktop')
if (-not $programs -or -not (Test-Path -LiteralPath $programs -PathType Container)) { throw 'The current user Start Menu folder is unavailable.' }
if (-not $desktop -or -not (Test-Path -LiteralPath $desktop -PathType Container)) { throw 'The current user Desktop folder is unavailable.' }
$powerShellPath = Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
if (-not (Test-Path -LiteralPath $powerShellPath -PathType Leaf)) { throw 'Windows PowerShell is unavailable for the uninstall shortcut.' }

New-Item -ItemType Directory -Force -Path $destination | Out-Null
if ([IO.Path]::GetFullPath($source) -ne [IO.Path]::GetFullPath($exe)) { Copy-Item -LiteralPath $source -Destination $exe -Force }
if ($sourceUninstaller -ne [IO.Path]::GetFullPath($uninstaller)) { Copy-Item -LiteralPath $sourceUninstaller -Destination $uninstaller -Force }

$shell = New-Object -ComObject WScript.Shell
foreach ($folder in @($desktop, $programs)) {
    $shortcut = $shell.CreateShortcut((Join-Path $folder 'Claude Mascot Rust.lnk'))
    $shortcut.TargetPath = $exe
    $shortcut.WorkingDirectory = $destination
    $shortcut.Save()
}
$removeShortcut = $shell.CreateShortcut((Join-Path $programs 'Удалить Claude Mascot Rust.lnk'))
$removeShortcut.TargetPath = $powerShellPath
$removeShortcut.Arguments = '-NoProfile -ExecutionPolicy Bypass -File "' + $uninstaller + '"'
$removeShortcut.WorkingDirectory = $destination
$removeShortcut.Save()

if ($Autostart) {
    New-ItemProperty -LiteralPath 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run' -Name ClaudeMascotRust -Value ('"' + $exe + '"') -PropertyType String -Force | Out-Null
}
Write-Host "Installed: $exe"
Write-Host 'Launch the desktop shortcut. Right-click the mascot or tray icon for controls.'
