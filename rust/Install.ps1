param([switch]$Autostart)
$ErrorActionPreference = 'Stop'
$source = Join-Path $PSScriptRoot 'claude-mascot.exe'
if (-not (Test-Path -LiteralPath $source)) { $source = Join-Path $PSScriptRoot 'target\release\claude-mascot.exe' }
if (-not (Test-Path -LiteralPath $source)) { throw 'Build with .\Build.ps1 or extract the portable release first.' }
$destination = Join-Path $env:LOCALAPPDATA 'claude-mascot-rust'
New-Item -ItemType Directory -Force -Path $destination | Out-Null
$exe = Join-Path $destination 'claude-mascot.exe'
if ([IO.Path]::GetFullPath($source) -ne [IO.Path]::GetFullPath($exe)) { Copy-Item -LiteralPath $source -Destination $exe -Force }
$shell = New-Object -ComObject WScript.Shell
foreach ($folder in @([Environment]::GetFolderPath('Desktop'),[Environment]::GetFolderPath('Programs'))) {
    $shortcut = $shell.CreateShortcut((Join-Path $folder 'Claude Mascot Rust.lnk'))
    $shortcut.TargetPath = $exe
    $shortcut.WorkingDirectory = $destination
    $shortcut.Save()
}
if ($Autostart) {
    New-ItemProperty -LiteralPath 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run' -Name ClaudeMascotRust -Value ('"' + $exe + '"') -PropertyType String -Force | Out-Null
}
Write-Host "Installed: $exe"
Write-Host 'Launch the desktop shortcut. Right-click the mascot or tray icon for controls.'
