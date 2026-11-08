@echo off
title Nintendo Switch 1 Game Extractor
cd /d "%~dp0"
python switch_extractor.py --gui %*
if %ERRORLEVEL% NEQ 0 (
    echo.
    echo Press any key to exit...
    pause >nul
)

