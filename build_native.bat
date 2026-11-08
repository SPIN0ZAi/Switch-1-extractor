@echo off
title Build Native Switch Extractor
echo ========================================================
echo   RUNNING AUTOMATED TEST SUITE (Rust Core Engine)
echo ========================================================
cargo test --manifest-path crates/switch-extractor-core/Cargo.toml
if %ERRORLEVEL% neq 0 (
    echo.
    echo [!] Automated tests failed! Build aborted.
    pause
    exit /b %ERRORLEVEL%
)

echo.
echo ========================================================
echo   ALL TESTS PASSED! COMPILING STANDALONE EXECUTABLE
echo ========================================================
call npm run tauri build -- --no-bundle
if %ERRORLEVEL% equ 0 (
    if not exist dist mkdir dist
    copy /Y src-tauri\target\release\switch-extractor-desktop.exe dist\SwitchExtractor.exe
    copy /Y dist\SwitchExtractor.exe Switch_Extractor.exe
    echo.
    echo ========================================================
    echo   BUILD SUCCESSFUL! Verified and updated Switch_Extractor.exe
    echo ========================================================
) else (
    echo.
    echo [!] Build failed!
)
pause
