# FindMaya.cmake - Find Maya SDK for multiple versions and platforms
# This module defines:
#  MAYA_FOUND - True if Maya SDK is found
#  MAYA_INCLUDE_DIRS - Include directories for Maya headers
#  MAYA_LIBRARIES - Libraries to link against
#  MAYA_LIBRARY_DIRS - Library directories
#  MAYA_VERSION_MAJOR - Maya major version
#  MAYA_VERSION_MINOR - Maya minor version
#  MAYA_PLUGIN_SUFFIX - Plugin file suffix (.mll, .so, .bundle)

cmake_minimum_required(VERSION 3.16)

# Maya version mapping
set(MAYA_VERSIONS_SUPPORTED 2018 2019 2020 2021 2022 2023 2024 2025 2026)

# Set default Maya version if not specified
if(NOT MAYA_VERSION)
    set(MAYA_VERSION "2024" CACHE STRING "Maya version to build for")
endif()

# Validate Maya version
if(NOT MAYA_VERSION IN_LIST MAYA_VERSIONS_SUPPORTED)
    message(FATAL_ERROR "Unsupported Maya version: ${MAYA_VERSION}. Supported versions: ${MAYA_VERSIONS_SUPPORTED}")
endif()

# Platform-specific Maya installation paths
if(WIN32)
    set(MAYA_INSTALL_BASE_PATH "C:/Program Files/Autodesk")
    set(MAYA_PLUGIN_SUFFIX ".mll")
    set(MAYA_LIBRARY_SUFFIX ".lib")
    set(MAYA_RUNTIME_SUFFIX ".dll")
elseif(APPLE)
    set(MAYA_INSTALL_BASE_PATH "/Applications/Autodesk")
    set(MAYA_PLUGIN_SUFFIX ".bundle")
    set(MAYA_LIBRARY_SUFFIX ".dylib")
    set(MAYA_RUNTIME_SUFFIX ".dylib")
else()
    set(MAYA_INSTALL_BASE_PATH "/usr/autodesk")
    set(MAYA_PLUGIN_SUFFIX ".so")
    set(MAYA_LIBRARY_SUFFIX ".so")
    set(MAYA_RUNTIME_SUFFIX ".so")
endif()

# Maya version-specific paths
if(MAYA_ROOT_DIR)
    # Use user-specified Maya root
    set(MAYA_BASE_DIR ${MAYA_ROOT_DIR})
else()
    # Auto-detect Maya installation
    set(MAYA_BASE_DIR "${MAYA_INSTALL_BASE_PATH}/Maya${MAYA_VERSION}")
endif()

# Platform-specific directory structure
if(WIN32)
    set(MAYA_INCLUDE_DIR "${MAYA_BASE_DIR}/include")
    set(MAYA_LIBRARY_DIR "${MAYA_BASE_DIR}/lib")
    set(MAYA_BIN_DIR "${MAYA_BASE_DIR}/bin")
elseif(APPLE)
    set(MAYA_INCLUDE_DIR "${MAYA_BASE_DIR}/Maya.app/Contents/include")
    set(MAYA_LIBRARY_DIR "${MAYA_BASE_DIR}/Maya.app/Contents/MacOS")
    set(MAYA_BIN_DIR "${MAYA_BASE_DIR}/Maya.app/Contents/bin")
else()
    set(MAYA_INCLUDE_DIR "${MAYA_BASE_DIR}/include")
    set(MAYA_LIBRARY_DIR "${MAYA_BASE_DIR}/lib")
    set(MAYA_BIN_DIR "${MAYA_BASE_DIR}/bin")
endif()

# Check if we're using Maya DevKit
# First check if MAYA_ROOT_DIR points to DevKit
if(MAYA_ROOT_DIR AND EXISTS "${MAYA_ROOT_DIR}/include/maya/MFn.h")
    get_filename_component(MAYA_INCLUDE_DIR "${MAYA_ROOT_DIR}/include" ABSOLUTE)
    get_filename_component(MAYA_LIBRARY_DIR "${MAYA_ROOT_DIR}/lib" ABSOLUTE)
    set(MAYA_USING_DEVKIT TRUE)
    message(STATUS "Using Maya DevKit from MAYA_ROOT_DIR: ${MAYA_ROOT_DIR}")
endif()

# Check if Maya SDK exists (if not using DevKit)
if(NOT MAYA_USING_DEVKIT AND NOT EXISTS "${MAYA_INCLUDE_DIR}")
    # Try Maya DevKit instead
    set(MAYA_DEVKIT_PATHS
        "${CMAKE_CURRENT_SOURCE_DIR}/maya-devkit/win"
        "${CMAKE_CURRENT_SOURCE_DIR}/maya-devkit/osx"
        "${CMAKE_CURRENT_SOURCE_DIR}/maya-devkit/linux"
        "${CMAKE_CURRENT_SOURCE_DIR}/maya-devkit"
    )

    foreach(DEVKIT_PATH ${MAYA_DEVKIT_PATHS})
        if(EXISTS "${DEVKIT_PATH}/include/maya/MFn.h")
            get_filename_component(MAYA_INCLUDE_DIR "${DEVKIT_PATH}/include" ABSOLUTE)
            get_filename_component(MAYA_LIBRARY_DIR "${DEVKIT_PATH}/lib" ABSOLUTE)
            set(MAYA_USING_DEVKIT TRUE)
            message(STATUS "Using Maya DevKit at: ${DEVKIT_PATH}")
            break()
        endif()
    endforeach()

    if(NOT MAYA_INCLUDE_DIR)
        message(WARNING "Maya SDK/DevKit not found at: ${MAYA_INCLUDE_DIR}")
        message(WARNING "Please install Maya ${MAYA_VERSION} SDK or ensure maya-devkit directory exists")
        set(MAYA_FOUND FALSE)
        return()
    endif()
endif()

# Maya libraries to link against
set(MAYA_CORE_LIBRARIES
    Foundation
    OpenMaya
    OpenMayaAnim
    OpenMayaFX
    OpenMayaRender
    OpenMayaUI
)

# Find Maya libraries (skip if using DevKit)
set(MAYA_LIBRARIES "")
if(NOT MAYA_USING_DEVKIT)
    foreach(MAYA_LIB ${MAYA_CORE_LIBRARIES})
        find_library(MAYA_${MAYA_LIB}_LIBRARY
            NAMES ${MAYA_LIB}
            PATHS ${MAYA_LIBRARY_DIR}
            NO_DEFAULT_PATH
        )

        if(MAYA_${MAYA_LIB}_LIBRARY)
            list(APPEND MAYA_LIBRARIES ${MAYA_${MAYA_LIB}_LIBRARY})
        else()
            message(WARNING "Maya library not found: ${MAYA_LIB}")
        endif()
    endforeach()
else()
    message(STATUS "Using Maya DevKit - no libraries to link")
endif()

# Maya version detection from headers
if(EXISTS "${MAYA_INCLUDE_DIR}/maya/MTypes.h")
    file(READ "${MAYA_INCLUDE_DIR}/maya/MTypes.h" MAYA_TYPES_CONTENT)
    
    # Extract version numbers
    string(REGEX MATCH "#define MAYA_API_VERSION ([0-9]+)" MAYA_API_VERSION_MATCH "${MAYA_TYPES_CONTENT}")
    if(MAYA_API_VERSION_MATCH)
        set(MAYA_API_VERSION ${CMAKE_MATCH_1})
        
        # Convert API version to major.minor
        math(EXPR MAYA_VERSION_MAJOR "${MAYA_API_VERSION} / 100")
        math(EXPR MAYA_VERSION_MINOR "${MAYA_API_VERSION} % 100")
        
        message(STATUS "Detected Maya API version: ${MAYA_API_VERSION} (${MAYA_VERSION_MAJOR}.${MAYA_VERSION_MINOR})")
    endif()
endif()

# Maya compiler definitions by version
set(MAYA_COMPILE_DEFINITIONS
    REQUIRE_IOSTREAM
    _BOOL
)

# Version-specific definitions
if(MAYA_VERSION GREATER_EQUAL 2018)
    list(APPEND MAYA_COMPILE_DEFINITIONS MAYA_2018_OR_LATER)
endif()

if(MAYA_VERSION GREATER_EQUAL 2020)
    list(APPEND MAYA_COMPILE_DEFINITIONS MAYA_2020_OR_LATER)
endif()

if(MAYA_VERSION GREATER_EQUAL 2022)
    list(APPEND MAYA_COMPILE_DEFINITIONS MAYA_2022_OR_LATER)
endif()

# Platform-specific definitions
if(WIN32)
    list(APPEND MAYA_COMPILE_DEFINITIONS
        NT_PLUGIN
        WIN32
        _WIN64
        _WINDOWS
        _USRDLL
        _CRT_SECURE_NO_WARNINGS
    )
elseif(APPLE)
    list(APPEND MAYA_COMPILE_DEFINITIONS
        OSMac_
        OSMacOSX_
        MAYA_OSX
    )
else()
    list(APPEND MAYA_COMPILE_DEFINITIONS
        LINUX
        _LINUX
        MAYA_LINUX
    )
endif()

# Set output variables
set(MAYA_INCLUDE_DIRS ${MAYA_INCLUDE_DIR})
set(MAYA_LIBRARY_DIRS ${MAYA_LIBRARY_DIR})

# Mark as found if we have the essentials
if(EXISTS "${MAYA_INCLUDE_DIR}" AND (MAYA_LIBRARIES OR MAYA_USING_DEVKIT))
    set(MAYA_FOUND TRUE)

    if(MAYA_USING_DEVKIT)
        message(STATUS "Found Maya DevKit ${MAYA_VERSION}")
        message(STATUS "  Include dir: ${MAYA_INCLUDE_DIR}")
        message(STATUS "  Plugin suffix: ${MAYA_PLUGIN_SUFFIX}")
        message(STATUS "  Mode: DevKit (headers only)")
    else()
        message(STATUS "Found Maya ${MAYA_VERSION}")
        message(STATUS "  Include dir: ${MAYA_INCLUDE_DIR}")
        message(STATUS "  Library dir: ${MAYA_LIBRARY_DIR}")
        message(STATUS "  Libraries: ${MAYA_LIBRARIES}")
        message(STATUS "  Plugin suffix: ${MAYA_PLUGIN_SUFFIX}")
    endif()
else()
    set(MAYA_FOUND FALSE)
    message(WARNING "Maya SDK/DevKit not found or incomplete")
endif()

# Create Maya target for easy linking
if(MAYA_FOUND AND NOT TARGET Maya::Maya)
    add_library(Maya::Maya INTERFACE IMPORTED)
    
    set_target_properties(Maya::Maya PROPERTIES
        INTERFACE_INCLUDE_DIRECTORIES "${MAYA_INCLUDE_DIRS}"
        INTERFACE_LINK_LIBRARIES "${MAYA_LIBRARIES}"
        INTERFACE_COMPILE_DEFINITIONS "${MAYA_COMPILE_DEFINITIONS}"
    )
    
    # Platform-specific link flags
    if(WIN32)
        set_target_properties(Maya::Maya PROPERTIES
            INTERFACE_LINK_OPTIONS "/export:initializePlugin;/export:uninitializePlugin"
        )
    elseif(APPLE)
        set_target_properties(Maya::Maya PROPERTIES
            INTERFACE_LINK_OPTIONS "-Wl,-exported_symbol,_initializePlugin;-Wl,-exported_symbol,_uninitializePlugin"
        )
    else()
        # Linux - will need version script
        set_target_properties(Maya::Maya PROPERTIES
            INTERFACE_LINK_OPTIONS "-Wl,--version-script=${CMAKE_CURRENT_SOURCE_DIR}/cmake/linux_plugin.map"
        )
    endif()
endif()

# Helper function to create Maya plugin
function(add_maya_plugin TARGET_NAME)
    cmake_parse_arguments(MAYA_PLUGIN "" "" "SOURCES;LIBRARIES" ${ARGN})
    
    if(NOT MAYA_FOUND)
        message(FATAL_ERROR "Maya SDK not found. Cannot create Maya plugin.")
    endif()
    
    # Create the plugin library
    add_library(${TARGET_NAME} SHARED ${MAYA_PLUGIN_SOURCES})
    
    # Set plugin properties
    set_target_properties(${TARGET_NAME} PROPERTIES
        PREFIX ""
        SUFFIX ${MAYA_PLUGIN_SUFFIX}
        OUTPUT_NAME ${TARGET_NAME}
    )
    
    # Link with Maya
    target_link_libraries(${TARGET_NAME} PRIVATE Maya::Maya ${MAYA_PLUGIN_LIBRARIES})
    
    # Set C++ standard
    target_compile_features(${TARGET_NAME} PRIVATE cxx_std_17)
    
    message(STATUS "Created Maya plugin target: ${TARGET_NAME}")
endfunction()

mark_as_advanced(
    MAYA_INCLUDE_DIR
    MAYA_LIBRARY_DIR
    MAYA_LIBRARIES
)
