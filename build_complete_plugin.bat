@echo off
REM Complete Umbrella Maya Plugin Build Script
REM This script builds both the Rust library and the C++ Maya plugin

echo ========================================
echo Building Complete Umbrella Maya Plugin
echo ========================================

REM Set build configuration
set BUILD_TYPE=Release
set MAYA_VERSION=2024
set MAYA_ROOT=C:\Program Files\Autodesk\Maya%MAYA_VERSION%

echo.
echo Build Configuration:
echo   Build Type: %BUILD_TYPE%
echo   Maya Version: %MAYA_VERSION%
echo   Maya Root: %MAYA_ROOT%
echo.

REM Check if Maya is installed
if not exist "%MAYA_ROOT%" (
    echo ERROR: Maya %MAYA_VERSION% not found at %MAYA_ROOT%
    echo Please install Maya %MAYA_VERSION% or update MAYA_VERSION variable
    pause
    exit /b 1
)

REM Create build directories
echo [1/5] Creating build directories...
if not exist "build" mkdir build
if not exist "build\include" mkdir build\include
if not exist "build\lib" mkdir build\lib
if not exist "build\plug-ins" mkdir build\plug-ins
if not exist "build_cpp" mkdir build_cpp

echo.
echo [2/5] Building Rust library...
echo ========================================

REM Build the Rust library
cargo build --release
if %ERRORLEVEL% neq 0 (
    echo ERROR: Failed to build Rust library
    pause
    exit /b 1
)

echo Rust library built successfully!

echo.
echo [3/5] Verifying C bindings...
echo ========================================

REM Check if the C bindings were generated
if not exist "build\include\umbrella_maya_plugin.h" (
    echo ERROR: C bindings not generated
    echo Please check cbindgen configuration
    pause
    exit /b 1
)

echo C bindings generated successfully!

echo.
echo [4/5] Building C++ Maya plugin...
echo ========================================

REM Configure CMake
cd build_cpp
cmake .. -DMAYA_VERSION=%MAYA_VERSION% -DMAYA_ROOT_DIR="%MAYA_ROOT%" -DCMAKE_BUILD_TYPE=%BUILD_TYPE%
if %ERRORLEVEL% neq 0 (
    echo ERROR: CMake configuration failed
    echo Please check Maya SDK installation and paths
    cd ..
    pause
    exit /b 1
)

REM Build the plugin
cmake --build . --config %BUILD_TYPE%
if %ERRORLEVEL% neq 0 (
    echo ERROR: C++ plugin build failed
    echo Please check compiler and Maya SDK setup
    cd ..
    pause
    exit /b 1
)

cd ..

echo C++ plugin built successfully!

echo.
echo [5/5] Finalizing build...
echo ========================================

REM Check if plugin was created
if not exist "build\plug-ins\UmbrellaMayaPlugin.mll" (
    echo ERROR: Plugin file not found
    echo Expected: build\plug-ins\UmbrellaMayaPlugin.mll
    pause
    exit /b 1
)

REM Check if Rust DLL was copied
if not exist "build\plug-ins\umbrella_maya_plugin.dll" (
    echo WARNING: Rust DLL not found in plugin directory
    echo Copying manually...
    copy "target\release\umbrella_maya_plugin.dll" "build\plug-ins\" >nul
    if %ERRORLEVEL% neq 0 (
        echo ERROR: Failed to copy Rust DLL
        pause
        exit /b 1
    )
)

echo.
echo ========================================
echo Build Summary
echo ========================================
echo ✅ Rust library: target\release\umbrella_maya_plugin.dll
echo ✅ C bindings:   build\include\umbrella_maya_plugin.h
echo ✅ C++ plugin:   build\plug-ins\UmbrellaMayaPlugin.mll
echo ✅ Rust DLL:     build\plug-ins\umbrella_maya_plugin.dll

echo.
echo Plugin Files:
dir "build\plug-ins\*.mll" /b 2>nul
dir "build\plug-ins\*.dll" /b 2>nul

echo.
echo ========================================
echo Installation Instructions
echo ========================================
echo 1. Copy the following files to Maya's plug-ins directory:
echo    - build\plug-ins\UmbrellaMayaPlugin.mll
echo    - build\plug-ins\umbrella_maya_plugin.dll
echo.
echo 2. Maya plug-ins directory locations:
echo    - Windows: %%USERPROFILE%%\Documents\maya\%MAYA_VERSION%\plug-ins\
echo    - Or: %MAYA_ROOT%\bin\plug-ins\
echo.
echo 3. Load the plugin in Maya:
echo    - Window ^> Settings/Preferences ^> Plug-in Manager
echo    - Find "UmbrellaMayaPlugin.mll" and check "Loaded"
echo.
echo 4. Test the plugin:
echo    - In Maya Script Editor: umbrellaInfo
echo    - Or: umbrellaScanScene
echo.

echo ========================================
echo Available Commands After Loading:
echo ========================================
echo   umbrellaScanFile [path]     - Scan a specific file
echo   umbrellaScanDirectory path  - Scan a directory
echo   umbrellaScanScene          - Scan current scene
echo   umbrellaStatus             - Show protection status
echo   umbrellaEnable             - Enable real-time protection
echo   umbrellaDisable            - Disable real-time protection
echo   umbrellaInfo               - Show plugin information

echo.
echo 🛡️ Build completed successfully!
echo Your Umbrella Maya Plugin is ready to protect Maya environments!
echo.
pause
