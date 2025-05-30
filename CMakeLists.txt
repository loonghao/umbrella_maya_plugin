cmake_minimum_required(VERSION 3.16)
project(UmbrellaMayaPlugin VERSION 1.0.0)

# Set C++ standard
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# Add custom CMake modules
list(APPEND CMAKE_MODULE_PATH "${CMAKE_CURRENT_SOURCE_DIR}/cmake")

# Configuration options
set(MAYA_VERSION "2024" CACHE STRING "Maya version to build for (2018-2026)")
set(RUST_TARGET "" CACHE STRING "Rust target triple (auto-detected if empty)")
option(BUILD_TESTS "Build test executables" ON)
option(ENABLE_ASAN "Enable AddressSanitizer" OFF)

# Validate Maya version
set(SUPPORTED_MAYA_VERSIONS 2018 2019 2020 2021 2022 2023 2024 2025 2026)
if(NOT MAYA_VERSION IN_LIST SUPPORTED_MAYA_VERSIONS)
    message(FATAL_ERROR "Unsupported Maya version: ${MAYA_VERSION}. Supported: ${SUPPORTED_MAYA_VERSIONS}")
endif()

# Find Maya SDK
find_package(Maya REQUIRED)

if(NOT MAYA_FOUND)
    message(FATAL_ERROR "Maya SDK not found for version ${MAYA_VERSION}")
endif()

# Auto-detect Rust target if not specified
if(NOT RUST_TARGET)
    if(WIN32)
        set(RUST_TARGET "x86_64-pc-windows-msvc")
    elseif(APPLE)
        if(CMAKE_SYSTEM_PROCESSOR MATCHES "arm64|aarch64")
            set(RUST_TARGET "aarch64-apple-darwin")
        else()
            set(RUST_TARGET "x86_64-apple-darwin")
        endif()
    else()
        set(RUST_TARGET "x86_64-unknown-linux-gnu")
    endif()
endif()

# Set output directories with version-specific naming
set(OUTPUT_DIR "${CMAKE_CURRENT_SOURCE_DIR}/dist/maya${MAYA_VERSION}")
set(CMAKE_RUNTIME_OUTPUT_DIRECTORY "${OUTPUT_DIR}")
set(CMAKE_LIBRARY_OUTPUT_DIRECTORY "${OUTPUT_DIR}")

# Rust library configuration
set(RUST_LIB_DIR "${CMAKE_CURRENT_SOURCE_DIR}/target/${RUST_TARGET}/release")
if(WIN32)
    set(RUST_LIB_NAME "umbrella_maya_plugin.dll")
    set(RUST_IMPORT_LIB "${RUST_LIB_DIR}/umbrella_maya_plugin.dll.lib")
elseif(APPLE)
    set(RUST_LIB_NAME "libumbrella_maya_plugin.dylib")
else()
    set(RUST_LIB_NAME "libumbrella_maya_plugin.so")
endif()

set(RUST_LIB_PATH "${RUST_LIB_DIR}/${RUST_LIB_NAME}")

# Include directories
include_directories(
    ${CMAKE_CURRENT_SOURCE_DIR}/build/include
)

# Create the Maya plugin using our helper function
add_maya_plugin(UmbrellaMayaPlugin
    SOURCES UmbrellaMayaPlugin.cpp
    LIBRARIES ${RUST_LIB_PATH}
)

# Version-specific plugin naming
set_target_properties(UmbrellaMayaPlugin PROPERTIES
    OUTPUT_NAME "UmbrellaMayaPlugin_${MAYA_VERSION}"
)

# AddressSanitizer support
if(ENABLE_ASAN)
    target_compile_options(UmbrellaMayaPlugin PRIVATE -fsanitize=address)
    target_link_options(UmbrellaMayaPlugin PRIVATE -fsanitize=address)
endif()

# Custom target to build Rust library first
add_custom_target(build_rust_lib
    COMMAND cargo build --release --target ${RUST_TARGET}
    WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
    COMMENT "Building Rust library for target ${RUST_TARGET}"
)

# Generate C bindings
add_custom_target(generate_bindings
    COMMAND cbindgen --config cbindgen.toml --crate umbrella_maya_plugin --output build/include/umbrella_maya_plugin.h
    WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
    COMMENT "Generating C bindings"
    DEPENDS build_rust_lib
)

# Make sure Rust library and bindings are built before the plugin
add_dependencies(UmbrellaMayaPlugin generate_bindings)

# Copy Rust library to output directory
add_custom_command(TARGET UmbrellaMayaPlugin POST_BUILD
    COMMAND ${CMAKE_COMMAND} -E copy_if_different
    ${RUST_LIB_PATH}
    ${OUTPUT_DIR}/${RUST_LIB_NAME}
    COMMENT "Copying Rust library to plugin directory"
)

# Create installation package
install(TARGETS UmbrellaMayaPlugin
    DESTINATION "maya${MAYA_VERSION}"
    COMPONENT maya_plugin
)

install(FILES ${RUST_LIB_PATH}
    DESTINATION "maya${MAYA_VERSION}"
    COMPONENT rust_library
)

# Create version-specific package
set(CPACK_PACKAGE_NAME "UmbrellaMayaPlugin")
set(CPACK_PACKAGE_VERSION "${PROJECT_VERSION}")
set(CPACK_PACKAGE_DESCRIPTION_SUMMARY "Umbrella Maya Plugin - Antivirus protection for Maya")
set(CPACK_PACKAGE_VENDOR "Umbrella Security Team")
set(CPACK_PACKAGE_FILE_NAME "UmbrellaMayaPlugin-${PROJECT_VERSION}-Maya${MAYA_VERSION}-${CMAKE_SYSTEM_NAME}")

include(CPack)

# Testing support
if(BUILD_TESTS)
    enable_testing()

    # Add basic plugin load test
    add_test(NAME plugin_load_test
        COMMAND ${CMAKE_COMMAND} -E echo "Plugin load test placeholder"
    )
endif()

# Print build information
message(STATUS "=== Umbrella Maya Plugin Build Configuration ===")
message(STATUS "Project Version: ${PROJECT_VERSION}")
message(STATUS "Maya Version: ${MAYA_VERSION}")
message(STATUS "Rust Target: ${RUST_TARGET}")
message(STATUS "Output Directory: ${OUTPUT_DIR}")
message(STATUS "Plugin Name: UmbrellaMayaPlugin_${MAYA_VERSION}${MAYA_PLUGIN_SUFFIX}")
message(STATUS "C++ Standard: ${CMAKE_CXX_STANDARD}")
message(STATUS "Build Tests: ${BUILD_TESTS}")
message(STATUS "AddressSanitizer: ${ENABLE_ASAN}")
message(STATUS "=================================================")
