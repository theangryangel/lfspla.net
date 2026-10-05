@echo off
LFS.exe /hlvc=lfsplanet_validator 1>&2
set "HLVC_RESULT=%ERRORLEVEL%"
echo %HLVC_RESULT% > Z:\sandbox\lfsplanet-hlvc-result
exit /b %HLVC_RESULT%
