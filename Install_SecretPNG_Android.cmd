@echo off
title Secret PNG - Android Installer
echo ========================================================
echo   Secret PNG - Android APK Installer
echo ========================================================
echo.

set ADB_PATH=C:\Users\ME\AppData\Local\Android\Sdk\platform-tools\adb.exe

if not exist "%ADB_PATH%" (
    where adb >nul 2>&1
    if %errorlevel% equ 0 (
        set ADB_PATH=adb
    ) else (
        echo [ERROR] ADB not found in SDK or PATH.
        echo Please copy 'dist\android\SecretPNG.apk' directly to your phone and tap to install.
        pause
        exit /b 1
    )
)

echo Checking for connected Android devices...
"%ADB_PATH%" devices

echo.
echo Installing SecretPNG.apk to device...
"%ADB_PATH%" install -r "%~dp0dist\android\SecretPNG.apk"

if %errorlevel% equ 0 (
    echo.
    echo [SUCCESS] Secret PNG has been installed on your Android device!
) else (
    echo.
    echo [NOTE] If no device was detected, make sure USB Debugging is ON in Android Developer Options,
    echo or simply copy 'dist\android\SecretPNG.apk' to your phone to install it.
)

echo.
pause
