@echo off
cd /d "%~dp0"
echo Iniciando proxy...
echo.
target\release\annes-token-proxy.exe init
echo.
echo Proxy encerrado. Pressione qualquer tecla para fechar...
pause >nul
