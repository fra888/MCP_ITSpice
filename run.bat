@echo off
setlocal
set "DIR=%~dp0"
if not exist "%DIR%target\release\MCP_ITSpice.exe" (
    if not exist "%DIR%target\debug\MCP_ITSpice.exe" (
        pushd "%DIR%"
        cargo build --release
        popd
    )
)
if exist "%DIR%target\release\MCP_ITSpice.exe" (
    "%DIR%target\release\MCP_ITSpice.exe" %*
) else (
    "%DIR%target\debug\MCP_ITSpice.exe" %*
)
