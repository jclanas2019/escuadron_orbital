@echo off
rem ============================================================
rem  ESCUADRON ORBITAL - arranque para Windows
rem  Doble clic en este archivo. La primera vez instala Rust (si
rem  falta) y compila el juego: tarda varios minutos. Despues
rem  arranca en segundos.
rem    start.bat --build   fuerza recompilar
rem ============================================================
setlocal EnableExtensions EnableDelayedExpansion
chcp 65001 >nul
cd /d "%~dp0"
title Escuadron Orbital

set "EXE=target\release\nave_combate.exe"
if /i not "%~1"=="--build" if exist "%EXE%" goto :jugar
if /i not "%~1"=="--build" if exist "bin\windows-x86_64\nave_combate.exe" (
    set "EXE=bin\windows-x86_64\nave_combate.exe"
    goto :jugar
)

rem ---- Rust ---------------------------------------------------
if exist "%USERPROFILE%\.cargo\bin\cargo.exe" set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
where cargo >nul 2>nul
if errorlevel 1 (
    echo [*] No se encontro Rust. Descargando el instalador oficial ^(rustup^)...
    powershell -NoProfile -ExecutionPolicy Bypass -Command "Invoke-WebRequest -UseBasicParsing https://win.rustup.rs/x86_64 -OutFile \"$env:TEMP\rustup-init.exe\""
    if not exist "%TEMP%\rustup-init.exe" (
        echo [!] No se pudo descargar rustup. Instalalo a mano desde https://rustup.rs y vuelve a ejecutar start.bat
        pause & exit /b 1
    )
    rem Con Visual Studio Build Tools se usa la cadena MSVC; si no, la GNU, que no necesita nada mas.
    set "HOST=x86_64-pc-windows-gnu"
    set "VSWHERE=%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe"
    if exist "!VSWHERE!" (
        for /f "usebackq delims=" %%i in (`"!VSWHERE!" -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath`) do set "HOST=x86_64-pc-windows-msvc"
    )
    echo [*] Instalando Rust ^(!HOST!^)...
    "%TEMP%\rustup-init.exe" -y --profile minimal --default-host !HOST!
    set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
) else (
    where rustup >nul 2>nul && (
        echo [*] Actualizando Rust...
        rustup update stable >nul 2>nul
    )
)

rem ---- compilar -----------------------------------------------
echo [*] Compilando el juego ^(la primera vez tarda varios minutos^)...
cargo build --release
if errorlevel 1 (
    echo [*] Reintentando sin la comprobacion de version de Rust...
    cargo build --release --ignore-rust-version
)
if errorlevel 1 (
    echo.
    echo [!] La compilacion fallo. Si usas la cadena GNU, prueba instalar
    echo     "Visual Studio Build Tools" ^(C++^) y ejecuta:  rustup default stable-msvc
    echo     y luego vuelve a ejecutar start.bat --build
    pause & exit /b 1
)

:jugar
set "NAVE_ASSETS=%~dp0assets"
echo [*] Arrancando Escuadron Orbital...
"%EXE%" %*
if errorlevel 1 pause
endlocal
