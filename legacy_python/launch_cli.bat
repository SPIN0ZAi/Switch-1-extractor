@echo off
title Nintendo Switch 1 Game Extractor CLI
cd /d "%~dp0"
python switch_extractor.py %*
if "%~1"=="" (
    pause
)

