@echo off
rem Compila o aplicativo (toolchain GNU do Rust + gcc do MSYS2 mingw64).
set "PATH=C:\msys64\mingw64\bin;%USERPROFILE%\.cargo\bin;%PATH%"
cd /d "%~dp0"
cargo build --release %*
