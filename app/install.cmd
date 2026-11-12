@echo off
rem Compila o aplicativo, copia para %LOCALAPPDATA%\AssistenteEmails, liga "Iniciar com o Windows" e abre na bandeja.
rem Pode ser executado de novo para atualizar (fecha a instancia em execucao antes de copiar).
setlocal
set "DEST=%LOCALAPPDATA%\AssistenteEmails"

call "%~dp0build.cmd" || exit /b 1

taskkill /im assistente-emails.exe /f >nul 2>&1
if not exist "%DEST%" mkdir "%DEST%"
copy /y "%~dp0target\release\assistente-emails.exe" "%DEST%\assistente-emails.exe" >nul || exit /b 1

start "" "%DEST%\assistente-emails.exe" --ativar-inicializacao --minimizado
echo Instalado em %DEST% (inicia com o Windows; abre na bandeja).
