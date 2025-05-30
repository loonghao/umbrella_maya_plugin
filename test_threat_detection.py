#!/usr/bin/env python3
"""
Simple test script for Umbrella threat detection functionality
"""

import ctypes
import os
import tempfile

# Define the structures
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

def test_threat_detection():
    """Test the threat detection functionality"""
    
    # Load the library
    dll_path = "target/release/umbrella_maya_plugin.dll"
    if not os.path.exists(dll_path):
        print(f"Error: DLL not found at {dll_path}")
        return False
    
    try:
        lib = ctypes.CDLL(dll_path)
        
        # Define function signatures
        lib.umbrella_init.restype = UmbrellaResult
        lib.umbrella_scan_file.restype = ScanResult
        lib.umbrella_scan_file.argtypes = [ctypes.c_char_p]
        lib.umbrella_cleanup.restype = UmbrellaResult
        
        print("=== Umbrella Threat Detection Test ===")
        
        # Initialize
        init_result = lib.umbrella_init()
        if not init_result.success:
            print("Failed to initialize")
            return False
        print("✅ Initialized successfully")
        
        # Test 1: Clean file
        print("\n1. Testing clean file...")
        with tempfile.NamedTemporaryFile(mode='w', suffix='.ma', delete=False) as f:
            f.write("""
// Clean Maya scene file
createNode transform -n "pCube1";
createNode mesh -n "pCubeShape1" -p "pCube1";
setAttr ".v" no;
""")
            clean_file = f.name
        
        result = lib.umbrella_scan_file(clean_file.encode('utf-8'))
        print(f"   Threats found: {result.threats_found}")
        print(f"   Files scanned: {result.files_scanned}")
        print(f"   Scan time: {result.scan_time_ms}ms")
        
        if result.threats_found == 0:
            print("   ✅ Clean file correctly identified")
        else:
            print("   ⚠️  False positive detected")
        
        # Test 2: Suspicious file
        print("\n2. Testing suspicious file...")
        with tempfile.NamedTemporaryFile(mode='w', suffix='.py', delete=False) as f:
            f.write("""
import os
import subprocess
import sys

# Suspicious code
exec("print('malicious code')")
eval("os.system('dir')")
subprocess.call(['notepad.exe'])

# Maya-specific suspicious patterns
import maya.cmds as cmds
cmds.evalDeferred("python('import os; os.system(\\"rm -rf /\\")')")
""")
            suspicious_file = f.name
        
        result = lib.umbrella_scan_file(suspicious_file.encode('utf-8'))
        print(f"   Threats found: {result.threats_found}")
        print(f"   Files scanned: {result.files_scanned}")
        print(f"   Scan time: {result.scan_time_ms}ms")
        
        if result.threats_found > 0:
            print("   ✅ Threats correctly detected")
        else:
            print("   ❌ Failed to detect threats")
        
        # Test 3: Non-existent file
        print("\n3. Testing non-existent file...")
        result = lib.umbrella_scan_file(b"non_existent_file.ma")
        print(f"   Threats found: {result.threats_found}")
        
        if result.threats_found == -1:
            print("   ✅ Error correctly handled")
        else:
            print("   ❌ Error handling failed")
        
        # Cleanup
        lib.umbrella_cleanup()
        os.unlink(clean_file)
        os.unlink(suspicious_file)
        
        print("\n🎉 Threat detection test completed!")
        return True
        
    except Exception as e:
        print(f"Error: {e}")
        return False

if __name__ == "__main__":
    test_threat_detection()
