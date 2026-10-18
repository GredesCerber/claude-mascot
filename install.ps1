# Ставит маскота в %LOCALAPPDATA%\claude-mascot и запускает его (Windows).
# Обычно запускается двойным кликом по install.bat.
#   install.bat              — спросит про автозапуск
#   install.bat -Autostart   — сразу с автозапуском
param([switch]$Autostart)
$ErrorActionPreference = 'Stop'

$Src = $PSScriptRoot
$Dest = Join-Path $env:LOCALAPPDATA 'claude-mascot'
$Pythonw = Join-Path $Dest 'venv\Scripts\pythonw.exe'
$VenvPython = Join-Path $Dest 'venv\Scripts\python.exe'
$Script = Join-Path $Dest 'mascot.py'
$LinkName = 'Claude-маскот.lnk'

function Find-Python {
    # "python" из Microsoft Store может оказаться заглушкой — проверяем, что он реально работает
    foreach ($candidate in 'py -3', 'python', 'python3') {
        $parts = $candidate -split ' '
        $cmd = $parts[0]
        $rest = @($parts | Select-Object -Skip 1)
        if (-not (Get-Command $cmd -ErrorAction SilentlyContinue)) { continue }
        try {
            $out = & $cmd @rest -c 'import sys; assert sys.version_info >= (3, 9); print(sys.executable)' 2>$null
        } catch { continue }
        $exe = "$($out | Select-Object -Last 1)".Trim()
        if ($LASTEXITCODE -eq 0 -and $exe -and (Test-Path $exe)) { return $exe }
    }
    return $null
}

function Stop-Mascot {
    Get-CimInstance Win32_Process -Filter "Name='pythonw.exe' OR Name='python.exe'" |
        Where-Object { $_.CommandLine -like '*claude-mascot*mascot.py*' } |
        ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
}

$Python = Find-Python
if (-not $Python) {
    Write-Host 'Не нашёл Python 3.9+.' -ForegroundColor Yellow
    if (Get-Command winget -ErrorAction SilentlyContinue) {
        $answer = Read-Host 'Поставить Python 3.12 через winget? [Y/n]'
        if ($answer -notmatch '^[nNнН]') {
            winget install -e --id Python.Python.3.12 --scope user
            $Python = Join-Path $env:LOCALAPPDATA 'Programs\Python\Python312\python.exe'
            if (-not (Test-Path $Python)) { $Python = Find-Python }
        }
    }
    if (-not $Python) {
        Write-Host 'Поставь Python с https://www.python.org/downloads/ (галочка "Add python.exe to PATH") и запусти install.bat ещё раз.' -ForegroundColor Red
        exit 1
    }
}
Write-Host "→ Python: $Python"

Write-Host "→ Копирую файлы в $Dest"
New-Item -ItemType Directory -Force -Path $Dest | Out-Null
Copy-Item -Force (Join-Path $Src 'mascot.py'), (Join-Path $Src 'platform_windows.py') $Dest
New-Item -ItemType Directory -Force -Path (Join-Path $Dest 'assets') | Out-Null
Copy-Item -Force (Join-Path $Src 'assets\tagilla_frames.png'), (Join-Path $Src 'assets\tagilla_frames.json') (Join-Path $Dest 'assets')

if (-not (Test-Path $VenvPython)) {
    Write-Host '→ Создаю виртуальное окружение'
    & $Python -m venv (Join-Path $Dest 'venv')
    if ($LASTEXITCODE -ne 0) { throw 'Не получилось создать venv' }
}

Write-Host '→ Ставлю PyQt6 (первый раз ~1 минута)'
& $VenvPython -m pip install -q --disable-pip-version-check --upgrade PyQt6
if ($LASTEXITCODE -ne 0) { throw 'Не получилось поставить PyQt6' }

Write-Host '→ Ставлю пакеты для подглядывания за видео'
& $VenvPython -m pip install -q --disable-pip-version-check --upgrade `
    winrt-runtime winrt-Windows.Foundation winrt-Windows.Foundation.Collections winrt-Windows.Media.Control
if ($LASTEXITCODE -ne 0) {
    Write-Host '⚠ Не встали пакеты winrt — маскот будет работать, но без подглядывания за видео.' -ForegroundColor Yellow
}

if (-not $Autostart) {
    $answer = Read-Host 'Запускать маскота при входе в Windows? [y/N]'
    $Autostart = $answer -match '^[yYдД]'
}

$Shell = New-Object -ComObject WScript.Shell
function New-MascotShortcut([string]$Folder) {
    $link = $Shell.CreateShortcut((Join-Path $Folder $LinkName))
    $link.TargetPath = $Pythonw
    $link.Arguments = "`"$Script`""
    $link.WorkingDirectory = $Dest
    $link.Description = 'Пиксельный маскот на рабочем столе'
    $link.Save()
}
New-MascotShortcut ([Environment]::GetFolderPath('Desktop'))
New-MascotShortcut ([Environment]::GetFolderPath('Programs'))
Write-Host '→ Ярлык на рабочем столе и в меню Пуск'
if ($Autostart) {
    New-MascotShortcut ([Environment]::GetFolderPath('Startup'))
    Write-Host '→ Добавил в автозапуск'
}

Write-Host '→ Запускаю'
Stop-Mascot
Start-Sleep -Milliseconds 500
Start-Process -FilePath $Pythonw -ArgumentList "`"$Script`"" -WorkingDirectory $Dest
Write-Host 'Готово! Маскот должен упасть сверху на экран.' -ForegroundColor Green
