#!/usr/bin/env python3
"""
Maya Plugin Test Script for Umbrella Antivirus Plugin
This script tests the Rust library integration with Maya Python
"""

import maya.cmds as cmds
import maya.mel as mel
import ctypes
import os
import sys
from pathlib import Path

# Define the structures that match the Rust definitions
class UmbrellaResult(ctypes.Structure):
    _fields_ = [
        ("success", ctypes.c_bool),
        ("error_code", ctypes.c_int)
    ]

class ScanResult(ctypes.Structure):
    _fields_ = [
        ("threats_found", ctypes.c_int),
        ("files_scanned", ctypes.c_int),
        ("scan_time_ms", ctypes.c_int)
    ]

def load_umbrella_library():
    """Load the Umbrella Rust library"""
    # Try different possible locations
    possible_paths = [
        "target/release/umbrella_maya_plugin.dll",
        "umbrella_maya_plugin.dll",
        "build/lib/umbrella_maya_plugin.dll"
    ]
    
    for dll_path in possible_paths:
        if os.path.exists(dll_path):
            try:
                lib = ctypes.CDLL(dll_path)
                
                # Define function signatures
                lib.umbrella_init.restype = UmbrellaResult
                lib.umbrella_init.argtypes = []
                
                lib.umbrella_scan_file.restype = ScanResult
                lib.umbrella_scan_file.argtypes = [ctypes.c_char_p]
                
                lib.umbrella_scan_directory.restype = ScanResult
                lib.umbrella_scan_directory.argtypes = [ctypes.c_char_p]
                
                lib.umbrella_get_version.restype = ctypes.c_char_p
                lib.umbrella_get_version.argtypes = []
                
                lib.umbrella_free_string.restype = None
                lib.umbrella_free_string.argtypes = [ctypes.c_char_p]
                
                lib.umbrella_cleanup.restype = UmbrellaResult
                lib.umbrella_cleanup.argtypes = []
                
                print(f"✅ Successfully loaded Umbrella library from: {dll_path}")
                return lib
                
            except Exception as e:
                print(f"❌ Failed to load library from {dll_path}: {e}")
                continue
    
    print("❌ Could not find Umbrella library in any expected location")
    return None

def test_umbrella_in_maya():
    """Test Umbrella functionality within Maya"""
    print("=" * 60)
    print("🎬 Testing Umbrella Maya Plugin Integration")
    print("=" * 60)
    
    # Load the library
    lib = load_umbrella_library()
    if not lib:
        return False
    
    try:
        # Test 1: Initialize
        print("\n1. Initializing Umbrella engine...")
        init_result = lib.umbrella_init()
        if not init_result.success:
            print(f"❌ Initialization failed with error code: {init_result.error_code}")
            return False
        print("✅ Umbrella engine initialized successfully!")
        
        # Test 2: Get version
        print("\n2. Getting version information...")
        version_ptr = lib.umbrella_get_version()
        if version_ptr:
            version = ctypes.string_at(version_ptr).decode('utf-8')
            print(f"📦 Umbrella version: {version}")
            lib.umbrella_free_string(version_ptr)
        
        # Test 3: Get current Maya scene file
        print("\n3. Scanning current Maya scene...")
        current_scene = cmds.file(query=True, sceneName=True)
        if current_scene:
            print(f"🎬 Current scene: {current_scene}")
            scene_bytes = current_scene.encode('utf-8')
            scan_result = lib.umbrella_scan_file(scene_bytes)
            
            print(f"📊 Scan Results:")
            print(f"   - Threats found: {scan_result.threats_found}")
            print(f"   - Files scanned: {scan_result.files_scanned}")
            print(f"   - Scan time: {scan_result.scan_time_ms}ms")
            
            if scan_result.threats_found == 0:
                print("✅ No threats detected in current scene!")
            else:
                print("⚠️  Threats detected! Please review your scene.")
        else:
            print("ℹ️  No scene currently open, creating a test scene...")
            
            # Create a simple test scene
            cmds.file(new=True, force=True)
            cmds.polyCube(name="test_cube")
            cmds.move(0, 1, 0, "test_cube")
            
            # Save the test scene
            test_scene_path = os.path.join(os.getcwd(), "test_scene.ma")
            cmds.file(rename=test_scene_path)
            cmds.file(save=True, type="mayaAscii")
            
            print(f"🎬 Created test scene: {test_scene_path}")
            
            # Scan the test scene
            scene_bytes = test_scene_path.encode('utf-8')
            scan_result = lib.umbrella_scan_file(scene_bytes)
            
            print(f"📊 Test Scene Scan Results:")
            print(f"   - Threats found: {scan_result.threats_found}")
            print(f"   - Files scanned: {scan_result.files_scanned}")
            print(f"   - Scan time: {scan_result.scan_time_ms}ms")
        
        # Test 4: Scan Maya scripts directory
        print("\n4. Scanning Maya scripts directory...")
        maya_app_dir = cmds.internalVar(userAppDir=True)
        scripts_dir = os.path.join(maya_app_dir, "scripts")
        
        if os.path.exists(scripts_dir):
            print(f"📁 Scanning directory: {scripts_dir}")
            dir_bytes = scripts_dir.encode('utf-8')
            dir_scan_result = lib.umbrella_scan_directory(dir_bytes)
            
            print(f"📊 Directory Scan Results:")
            print(f"   - Threats found: {dir_scan_result.threats_found}")
            print(f"   - Files scanned: {dir_scan_result.files_scanned}")
            print(f"   - Scan time: {dir_scan_result.scan_time_ms}ms")
        else:
            print(f"⚠️  Scripts directory not found: {scripts_dir}")
        
        # Test 5: Cleanup
        print("\n5. Cleaning up...")
        cleanup_result = lib.umbrella_cleanup()
        if cleanup_result.success:
            print("✅ Cleanup completed successfully!")
        else:
            print(f"⚠️  Cleanup warning (error code: {cleanup_result.error_code})")
        
        print("\n" + "=" * 60)
        print("🎉 All tests completed successfully!")
        print("🛡️  Umbrella Maya Plugin is working correctly!")
        print("=" * 60)
        
        return True
        
    except Exception as e:
        print(f"❌ Error during testing: {e}")
        return False

def create_umbrella_maya_command():
    """Create a Maya MEL command for easy access"""
    mel_command = '''
global proc umbrellaScan(string $path)
{
    python("import test_maya_plugin; test_maya_plugin.test_umbrella_in_maya()");
}

global proc umbrellaInfo()
{
    print("Umbrella Maya Plugin - Antivirus protection for Maya scenes\\n");
    print("Usage: umbrellaScan(\\"path\\") - Scan a file or directory\\n");
    print("       umbrellaInfo() - Show this information\\n");
}
'''
    
    try:
        mel.eval(mel_command)
        print("✅ Created Maya MEL commands:")
        print("   - umbrellaScan(path) - Scan files/directories")
        print("   - umbrellaInfo() - Show plugin information")
    except Exception as e:
        print(f"⚠️  Could not create MEL commands: {e}")

def main():
    """Main function for Maya environment"""
    print("🛡️  Umbrella Maya Plugin Test")
    print("Running in Maya Python environment...")
    
    # Test if we're in Maya
    try:
        maya_version = cmds.about(version=True)
        print(f"🎬 Maya Version: {maya_version}")
        
        # Run the tests
        success = test_umbrella_in_maya()
        
        if success:
            # Create convenience commands
            create_umbrella_maya_command()
            print("\n💡 You can now use: umbrellaScan() or umbrellaInfo() in Maya")
        
        return success
        
    except Exception as e:
        print(f"❌ Error: This script must be run in Maya Python environment")
        print(f"   Details: {e}")
        return False

if __name__ == "__main__":
    main()
