param([switch]$RemoveSettings)
$ErrorActionPreference = 'Stop'
$destination = [IO.Path]::GetFullPath((Join-Path $env:LOCALAPPDATA 'claude-mascot-rust'))
$expectedParent = [IO.Path]::GetFullPath($env:LOCALAPPDATA).TrimEnd('\') + '\'
if (-not $destination.StartsWith($expectedParent,[StringComparison]::OrdinalIgnoreCase) -or [IO.Path]::GetFileName($destination) -ne 'claude-mascot-rust') { throw 'Unexpected installation directory' }
$exe = Join-Path $destination 'claude-mascot.exe'
Get-Process -Name claude-mascot -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $exe } | Stop-Process
Remove-ItemProperty -LiteralPath 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run' -Name ClaudeMascotRust -ErrorAction SilentlyContinue
foreach ($folder in @([Environment]::GetFolderPath('Desktop'),[Environment]::GetFolderPath('Programs'))) {
    $shortcut = Join-Path $folder 'Claude Mascot Rust.lnk'
    if (Test-Path -LiteralPath $shortcut) { Remove-Item -LiteralPath $shortcut }
}
if (Test-Path -LiteralPath $exe) { Remove-Item -LiteralPath $exe }
if ($RemoveSettings -and (Test-Path -LiteralPath $destination)) { Remove-Item -LiteralPath $destination -Recurse }
Write-Host 'Rust mascot removed. The Python version is separate.'
