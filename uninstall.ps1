# Останавливает маскота и удаляет всё, что поставил install.ps1 (Windows).
$Dest = Join-Path $env:LOCALAPPDATA 'claude-mascot'
$LinkName = 'Claude-маскот.lnk'

Get-CimInstance Win32_Process -Filter "Name='pythonw.exe' OR Name='python.exe'" |
    Where-Object { $_.CommandLine -like '*claude-mascot*mascot.py*' } |
    ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
Start-Sleep -Milliseconds 500

foreach ($folder in 'Desktop', 'Programs', 'Startup') {
    $link = Join-Path ([Environment]::GetFolderPath($folder)) $LinkName
    if (Test-Path $link) { Remove-Item -Force $link }
}
if (Test-Path $Dest) { Remove-Item -Recurse -Force $Dest }
Write-Host 'Маскот удалён.' -ForegroundColor Green
