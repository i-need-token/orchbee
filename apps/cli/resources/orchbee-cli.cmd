@echo off
setlocal
set "ORCHBEE_CLI_ENTRY_PATH=%~dp0orchbee-cli.cjs"
set "ORCHBEE_CLI_NATIVE_ADDON_PATH=%~dp0orchbee-semantic-command-endpoint.node"

if exist "%ORCHBEE_CLI_ENTRY_PATH%" (
	set "ELECTRON_RUN_AS_NODE=1"
	set "ORCHBEE_CLI_EXECUTABLE=%~dp0..\Orchbee.exe"
) else (
	set "ORCHBEE_CLI_ENTRY_PATH=%~dp0..\dist\main\orchbee-cli.cjs"
	set "ORCHBEE_CLI_NATIVE_ADDON_PATH=%~dp0..\..\desktop\zig-out\lib\orchbee-semantic-command-endpoint.node"
	set "ORCHBEE_CLI_EXECUTABLE=node.exe"
)

"%ORCHBEE_CLI_EXECUTABLE%" "%ORCHBEE_CLI_ENTRY_PATH%" %*
exit /b %ERRORLEVEL%
