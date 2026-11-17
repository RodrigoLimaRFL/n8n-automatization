@echo off
rem Compila o aplicativo, instala em %USERPROFILE%\AssistenteEmails, liga "Iniciar com o Windows"
rem (tarefa agendada de logon) e abre o app na bandeja. Pode ser executado de novo para atualizar.
rem
rem Importante: NAO instale em %LOCALAPPDATA% nem use a chave Run do registro quando este script
rem for executado de dentro de um app empacotado (como o app do Claude): essas escritas ficam numa
rem copia privada do pacote e o Windows nao as enxerga no logon.
setlocal
set "DEST=%USERPROFILE%\AssistenteEmails"
set "TASK=Assistente de Emails"

call "%~dp0build.cmd" || exit /b 1

taskkill /im assistente-emails.exe /f >nul 2>&1
if not exist "%DEST%" mkdir "%DEST%"
rem O processo encerrado pode levar alguns segundos para liberar o exe: tenta copiar ate 10 vezes.
set /a TRIES=0
:copy
copy /y "%~dp0target\release\assistente-emails.exe" "%DEST%\assistente-emails.exe" >nul 2>&1 && goto copied
set /a TRIES+=1
if %TRIES% geq 10 (echo Nao foi possivel substituir o exe: ele ainda esta em uso. & exit /b 1)
ping -n 2 127.0.0.1 >nul
goto copy
:copied

rem Registra a tarefa de logon (o proprio app faz isso) e encerra essa instancia temporaria.
start "" "%DEST%\assistente-emails.exe" --ativar-inicializacao --minimizado
ping -n 5 127.0.0.1 >nul
taskkill /im assistente-emails.exe /f >nul 2>&1

rem Inicia pela tarefa: o app roda como um processo normal do Windows, fora de qualquer pacote.
schtasks /Run /TN "%TASK%" >nul || exit /b 1
echo Instalado em %DEST% (tarefa "%TASK%": inicia com o Windows e abre na bandeja).
