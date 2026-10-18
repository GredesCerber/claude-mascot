@echo off
rem Двойной клик — установить и запустить маскота. Можно передать -Autostart.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0install.ps1" %*
pause
