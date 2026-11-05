param([switch]$RemoveSettings)
$ErrorActionPreference = 'Stop'
$destination = [IO.Path]::GetFullPath((Join-Path $env:LOCALAPPDATA 'claude-mascot-rust'))
$expectedParent = [IO.Path]::GetFullPath($env:LOCALAPPDATA).TrimEnd('\') + '\'
if (-not $destination.StartsWith($expectedParent,[StringComparison]::OrdinalIgnoreCase) -or [IO.Path]::GetFileName($destination) -ne 'claude-mascot-rust') { throw 'Unexpected installation directory' }
$marker = Join-Path $destination 'install-mode-msi.marker'
if (Test-Path -LiteralPath $marker -PathType Leaf) {
    throw 'This directory is owned by the MSI installation. Uninstall it through Windows Settings or Apps & Features.'
}

$exe = Join-Path $destination 'claude-mascot.exe'
$uninstaller = Join-Path $destination 'Uninstall.ps1'
$matchingProcesses = @(Get-Process -Name claude-mascot -ErrorAction SilentlyContinue | Where-Object {
    try { [IO.Path]::GetFullPath($_.Path).Equals($exe, [StringComparison]::OrdinalIgnoreCase) } catch { $false }
})
foreach ($process in $matchingProcesses) {
    try { [void]$process.CloseMainWindow() } catch { }
}
foreach ($process in $matchingProcesses) {
    try {
        if (-not $process.WaitForExit(5000)) { Stop-Process -Id $process.Id -Force -ErrorAction Stop }
    } catch {
        if (Get-Process -Id $process.Id -ErrorAction SilentlyContinue) { throw "Could not stop Claude Mascot process $($process.Id): $_" }
    }
}

$runPath = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
$runEntry = Get-ItemProperty -LiteralPath $runPath -Name ClaudeMascotRust -ErrorAction SilentlyContinue
if ($null -ne $runEntry) {
    $runCommand = [string]$runEntry.ClaudeMascotRust
    $match = [regex]::Match($runCommand, '^\s*"(?<exe>[^"]+)"(?:\s|$)')
    if ($match.Success -and [IO.Path]::GetFullPath($match.Groups['exe'].Value).Equals($exe, [StringComparison]::OrdinalIgnoreCase)) {
        Remove-ItemProperty -LiteralPath $runPath -Name ClaudeMascotRust -ErrorAction Stop
    }
}

function Test-ShortcutTarget($ShortcutPath, $ExpectedTarget) {
    try {
        $link = $shell.CreateShortcut($ShortcutPath)
        return [IO.Path]::GetFullPath($link.TargetPath).Equals($ExpectedTarget, [StringComparison]::OrdinalIgnoreCase)
    } catch {
        return $false
    }
}

$shell = New-Object -ComObject WScript.Shell
$desktop = [Environment]::GetFolderPath('Desktop')
$programs = [Environment]::GetFolderPath('Programs')
foreach ($folder in @($desktop, $programs)) {
    $shortcut = Join-Path $folder 'Claude Mascot Rust.lnk'
    if ((Test-Path -LiteralPath $shortcut -PathType Leaf) -and (Test-ShortcutTarget $shortcut $exe)) {
        Remove-Item -LiteralPath $shortcut -Force
    }
}
$uninstallShortcut = Join-Path $programs 'Удалить Claude Mascot Rust.lnk'
if (Test-Path -LiteralPath $uninstallShortcut -PathType Leaf) {
    try {
        $link = $shell.CreateShortcut($uninstallShortcut)
        $fileArg = [regex]::Match([string]$link.Arguments, '(?i)(?:^|\s)-File\s+"(?<path>[^"]+)"(?:\s|$)')
        if ($fileArg.Success -and [IO.Path]::GetFullPath($fileArg.Groups['path'].Value).Equals($uninstaller, [StringComparison]::OrdinalIgnoreCase)) {
            Remove-Item -LiteralPath $uninstallShortcut -Force
        }
    } catch { }
}
foreach ($file in @($exe, $uninstaller)) {
    if (Test-Path -LiteralPath $file -PathType Leaf) { Remove-Item -LiteralPath $file -Force }
}
if ($RemoveSettings) {
    foreach ($name in @('settings.json', 'last-error.log')) {
        $file = Join-Path $destination $name
        if (Test-Path -LiteralPath $file -PathType Leaf) { Remove-Item -LiteralPath $file -Force }
    }
}
if ((Test-Path -LiteralPath $destination -PathType Container) -and -not (Get-ChildItem -LiteralPath $destination -Force | Select-Object -First 1)) {
    Remove-Item -LiteralPath $destination -Force
}
Write-Host 'Rust mascot removed. Other user files and the Python version are preserved.'
