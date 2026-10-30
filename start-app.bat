@echo off
title SetupPal: Zero-Trust Desktop App (dev)
cd /d "%~dp0"
if not exist "crates\gui\node_modules" (
    call npm --prefix crates/gui install || exit /b 1
)
call npm --prefix crates/gui run app
