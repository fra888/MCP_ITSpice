@echo off
setlocal
set "DIR=%~dp0"
if not exist "%DIR%target\debug\MCP_ITSpice.exe" (
    pushd "%DIR%"
    cargo build --quiet 1>&2
    popd
)
"%DIR%target\debug\MCP_ITSpice.exe" --mcp %*
