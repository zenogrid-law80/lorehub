@echo off
setlocal EnableExtensions

rem Run this file once from an elevated Command Prompt on the Windows runner.
rem It installs the MSVC toolchain needed by Rust's x86_64-pc-windows-msvc target
rem and places Rust in ProgramData so the SYSTEM scheduled task can use it.

net session >nul 2>&1
if errorlevel 1 (
    echo Run this script from an elevated Command Prompt.
    exit /b 1
)

call :install_build_tools
if errorlevel 1 exit /b 1

call :install_rust
if errorlevel 1 exit /b 1

echo Windows server build prerequisites are ready.
exit /b 0

:install_build_tools
where winget >nul 2>&1
if errorlevel 1 (
    echo winget was not found. Install Visual Studio Build Tools manually.
    exit /b 1
)

set "VSWHERE=%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe"
if exist "%VSWHERE%" (
    for /f "usebackq delims=" %%I in (`"%VSWHERE%" -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath 2^>nul`) do (
        if exist "%%~fI\VC\Auxiliary\Build\vcvars64.bat" set "MSVC_ROOT=%%~fI"
    )
    if defined MSVC_ROOT (
        echo Visual Studio C++ Build Tools are already installed.
        exit /b 0
    )
)

rem Visual Studio Installer may find a per-user/new-version instance while the
rem LoreHub Runner SYSTEM account cannot discover it through vswhere. Accept a
rem machine-wide instance only when its MSVC environment script is readable.
for /d %%V in ("%ProgramFiles%\Microsoft Visual Studio\*\*") do (
    if exist "%%~fV\VC\Auxiliary\Build\vcvars64.bat" set "MSVC_ROOT=%%~fV"
)
if defined MSVC_ROOT (
    echo Visual Studio C++ Build Tools are installed at %MSVC_ROOT%.
    echo The LoreHub Runner SYSTEM account must be able to read this path.
    exit /b 0
)

echo Installing Visual Studio 2022 Build Tools and the Windows SDK...
winget install --id Microsoft.VisualStudio.2022.BuildTools --exact --source winget --accept-source-agreements --accept-package-agreements --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --add Microsoft.VisualStudio.Component.Windows11SDK.22621"
if errorlevel 1 (
    echo Visual Studio Build Tools installation failed. An administrator may see Visual Studio while the LoreHub Runner SYSTEM account still cannot discover its C++ toolchain.
    exit /b 1
)
exit /b 0

:install_rust
set "TOOLCHAIN_ROOT=C:\ProgramData\LoreHub\toolchains"
set "CARGO_HOME=%TOOLCHAIN_ROOT%\.cargo"
set "RUSTUP_HOME=%TOOLCHAIN_ROOT%\.rustup"
set "PATH=%CARGO_HOME%\bin;%PATH%"

if exist "%CARGO_HOME%\bin\cargo.exe" (
    echo Rust is already installed for the SYSTEM runner.
    exit /b 0
)

echo Installing Rust for the SYSTEM runner...
set "RUSTUP_INIT=%TEMP%\rustup-init-%RANDOM%.exe"
curl.exe --proto "=https" --tlsv1.2 -sSf -o "%RUSTUP_INIT%" https://win.rustup.rs/x86_64
if errorlevel 1 (
    echo Failed to download the Rust installer.
    exit /b 1
)

"%RUSTUP_INIT%" -y --default-toolchain stable-x86_64-pc-windows-msvc
set "INSTALL_RESULT=%ERRORLEVEL%"
del /q "%RUSTUP_INIT%" >nul 2>&1
if not "%INSTALL_RESULT%"=="0" (
    echo Rust installation failed.
    exit /b %INSTALL_RESULT%
)
exit /b 0
