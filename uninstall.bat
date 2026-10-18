@echo off
rem Двойной клик — удалить маскота.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0uninstall.ps1"
pause
