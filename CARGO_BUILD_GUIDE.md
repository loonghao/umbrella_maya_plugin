# Cargo Maya Build - Pure Rust Build Tool User Guide

## 🎯 Overview

We have created a pure Rust cross-platform Maya plugin build tool that enables one-click building of Maya plugins for all platforms using the `cargo maya-build` command.

## 🛠️ Features

### ✅ Core Functionality
- **Cross-platform Building**: Windows (.mll), Linux (.so), macOS (.bundle)
- **Multi-version Support**: Maya 2018, 2020, 2022, 2023, 2024
- **Automated Workflow**: Automatic Maya DevKit download, Rust target installation, building and packaging
- **Smart Caching**: Avoids redundant downloads and builds
- **Detailed Logging**: Colorized output with comprehensive build information

### 🎨 User Experience
- **Colorized Output**: Uses emojis and colors to distinguish different message types
- **Progress Display**: Clear build progress and status feedback
- **Error Handling**: Friendly error messages and suggestions
- **Flexible Configuration**: Supports various build options and combinations

## 📦 Installation and Setup

### 1. Build Tool
```bash
# Build the cargo-maya-build tool
cargo build --bin cargo-maya-build --release

# Or run directly
cargo run --bin cargo-maya-build -- --help
```

### 2. System Requirements
- **Rust**: 1.70+ (with async/await support)
- **CMake**: 3.16+ (for C++ plugin compilation)
- **Git**: For Maya DevKit download
- **Network Connection**: Required for initial Maya DevKit download

### 3. Platform-specific Requirements

#### Windows
- Visual Studio 2019+ or Build Tools
- Windows SDK

#### Linux
- GCC or Clang
- Development tools: `sudo apt-get install build-essential cmake`

#### macOS
- Xcode Command Line Tools
- CMake: `brew install cmake`

## 🚀 Usage

### Basic Usage

#### 1. Build Default Version for Current Platform (Maya 2024)
```bash
cargo maya-build
```

#### 2. Build Specific Platform and Version
```bash
# Windows Maya 2024
cargo maya-build --platform windows --maya-version 2024

# Linux Maya 2023
cargo maya-build --platform linux --maya-version 2023

# macOS Maya 2022
cargo maya-build --platform macos --maya-version 2022
```

#### 3. Build All Platforms and Versions
```bash
# Build all supported platforms and versions
cargo maya-build --all-platforms --all-versions

# Build all versions (current platform only)
cargo maya-build --all-versions

# Build all platforms (Maya 2024 only)
cargo maya-build --all-platforms
```

### Advanced Options

#### 1. Skip Specific Build Steps
```bash
# Build Rust library only, skip C++ plugin
cargo maya-build --skip-cpp

# Build C++ plugin only, skip Rust library
cargo maya-build --skip-rust

# Build current platform only
cargo maya-build --current-only
```

#### 2. Verbose Output and Cleanup
```bash
# Verbose output
cargo maya-build --verbose

# Clean build directories
cargo maya-build --clean
```

#### 3. Combined Usage
```bash
# Verbose build for all platforms with Maya 2024
cargo maya-build --all-platforms --maya-version 2024 --verbose

# Clean and build all versions for current platform
cargo maya-build --clean && cargo maya-build --all-versions
```

## 📁 Output Structure

### Build Artifacts
```
dist/
├── maya2018-windows/
│   ├── UmbrellaMayaPlugin_2018.mll
│   ├── umbrella_maya_plugin.dll
│   └── VERSION.txt
├── maya2024-linux/
│   ├── UmbrellaMayaPlugin_2024.so
│   ├── libumbrella_maya_plugin.so
│   └── VERSION.txt
└── maya2024-macos/
    ├── UmbrellaMayaPlugin_2024.bundle
    ├── libumbrella_maya_plugin.dylib
    └── VERSION.txt
```

### Temporary Files
```
maya-devkit/          # Maya DevKit (auto-downloaded)
build_windows_2024/   # CMake build directory
build_linux_2024/    # CMake build directory
target/               # Rust build directory
```

## 🎨 Output Examples

### Successful Build
```
🚀 Starting Umbrella Maya Plugin build...
🎯 Target platforms: [Windows]
🎯 Target Maya versions: ["2024"]
📦 Setting up Maya DevKit...
✅ Maya DevKit already exists
🦀 Installing Rust targets...
✅ Installed: x86_64-pc-windows-msvc
🦀 Building Rust library for windows...
✅ Rust library built for windows

============================================================
Building: Windows Maya 2024
============================================================
🏗️ Building Maya plugin for windows Maya 2024...
✅ Maya plugin built for windows Maya 2024
📦 Packaging artifacts for windows Maya 2024...
✅ Artifacts packaged in: dist\maya2024-windows
✅ Windows Maya 2024 completed

============================================================
🎉 Build Summary
============================================================
✅ Successful builds: 1/1
📁 Output directory: dist

📦 Built packages:
  📂 maya2024-windows (3 files)

✅ All builds completed successfully!
```

### Error Handling
```
❌ CMake configuration failed: CMake Error: Could not find CMAKE_C_COMPILER
💡 Please install Visual Studio Build Tools or Xcode Command Line Tools
```

## 🔧 Troubleshooting

### Common Issues

#### 1. Maya DevKit Download Failed
```bash
# Manual download and extraction
curl -L https://github.com/sonictk/Maya-devkit/archive/refs/heads/master.zip -o maya-devkit.zip
unzip maya-devkit.zip
mv Maya-devkit-master maya-devkit
```

#### 2. CMake Not Found
```bash
# Windows (using Chocolatey)
choco install cmake

# macOS
brew install cmake

# Linux
sudo apt-get install cmake
```

#### 3. Rust Target Not Installed
```bash
# Manually install targets
rustup target add x86_64-pc-windows-msvc
rustup target add x86_64-unknown-linux-gnu
rustup target add x86_64-apple-darwin
```

#### 4. Build Failed
```bash
# Clean and rebuild
cargo maya-build --clean
cargo maya-build --verbose
```

### Debugging Tips

#### 1. Verbose Logging
```bash
# Enable verbose output to see detailed errors
cargo maya-build --verbose
```

#### 2. Step-by-step Building
```bash
# Build Rust library only
cargo maya-build --skip-cpp

# Build C++ plugin only
cargo maya-build --skip-rust
```

#### 3. Environment Check
```bash
# Check Rust toolchain
rustup show

# Check CMake
cmake --version

# Check compiler
gcc --version  # Linux
clang --version  # macOS
```

## 📊 Performance Optimization

### Build Times
- **Initial Build**: ~10-15 minutes (including DevKit download)
- **Incremental Build**: ~3-5 minutes (utilizing cache)
- **Parallel Build**: Supports multi-platform parallel builds (manually start multiple processes)

### Caching Strategy
- **Maya DevKit**: Download once, use permanently
- **Rust Dependencies**: Automatically cached by Cargo
- **CMake Build**: Incremental compilation

## 🎉 Summary

Through the `cargo maya-build` tool, we have achieved:

1. **✅ Pure Rust Implementation**: No Python dependencies required
2. **✅ One-click Build**: From source code to usable plugin
3. **✅ Cross-platform Support**: Windows/Linux/macOS
4. **✅ Multi-version Support**: Maya 2018-2024
5. **✅ User-friendly**: Colorized output and detailed feedback
6. **✅ Highly Configurable**: Flexible build options

This tool provides a **modern, efficient, and reliable** build solution for Maya plugin development!

---

**🛡️ Make Maya plugin building simple with `cargo maya-build`!**
