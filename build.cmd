@echo off
setlocal enabledelayedexpansion

echo ========================================================
echo       Cat Codex Switcher - Release Build Script
echo ========================================================
echo.

:: 1. Check Node.js
where node >nul 2>nul
if %ERRORLEVEL% neq 0 (
    echo [ERROR] Node.js is not found in PATH!
    pause
    exit /b 1
)

:: 2. Check Rust / Cargo
where cargo >nul 2>nul
if %ERRORLEVEL% neq 0 (
    echo [ERROR] Cargo / Rust is not found in PATH!
    pause
    exit /b 1
)

:: 3. Install npm dependencies if missing
if not exist "node_modules" (
    echo [*] Installing npm frontend dependencies...
    call npm.cmd install
    if %ERRORLEVEL% neq 0 (
        echo [ERROR] npm install failed!
        pause
        exit /b 1
    )
)

:: 4. Build standalone release binary via Tauri
echo.
echo [*] Building standalone release binary (Tauri v2 + React)...
call npx.cmd tauri build --no-bundle
if %ERRORLEVEL% neq 0 (
    echo [ERROR] Tauri build failed!
    pause
    exit /b 1
)

:: 5. Create output directory and copy binary
if not exist "releases-comparison" mkdir "releases-comparison"

set "TARGET_EXE=src-tauri\target\release\Cat-Codex-Switcher.exe"
if not exist "%TARGET_EXE%" set "TARGET_EXE=target\release\Cat-Codex-Switcher.exe"

if exist "%TARGET_EXE%" (
    copy /Y "%TARGET_EXE%" "releases-comparison\Cat-Codex-Switcher-NEW-Tauri.exe" >nul
    echo.
    echo [SUCCESS] Release binary saved to: releases-comparison\Cat-Codex-Switcher-NEW-Tauri.exe
) else (
    echo [ERROR] Build output executable not found!
    pause
    exit /b 1
)

:: 6. Clean up heavy caches and node_modules
echo.
echo [*] Cleaning up heavy build caches and node_modules...
if exist "src-tauri\target" rmdir /s /q "src-tauri\target"
if exist "target" rmdir /s /q "target"
if exist "node_modules" rmdir /s /q "node_modules"
if exist "dist" rmdir /s /q "dist"

echo.
echo ========================================================
echo     Build and cleanup completed successfully!
echo ========================================================
echo.
pause
