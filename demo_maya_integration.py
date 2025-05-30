#!/usr/bin/env python3
"""
Umbrella Maya Plugin - 实际集成演示
这个脚本展示如何在真实的Maya环境中使用Umbrella反病毒插件
"""

import maya.cmds as cmds
import maya.mel as mel
import ctypes
import os
import tempfile

# 定义结构体
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

class UmbrellaMayaIntegration:
    """Umbrella Maya 插件集成类"""
    
    def __init__(self):
        self.lib = None
        self.initialized = False
        
    def load_library(self):
        """加载 Umbrella Rust 库"""
        possible_paths = [
            "target/release/umbrella_maya_plugin.dll",
            "umbrella_maya_plugin.dll",
            "build/lib/umbrella_maya_plugin.dll"
        ]
        
        for dll_path in possible_paths:
            if os.path.exists(dll_path):
                try:
                    self.lib = ctypes.CDLL(dll_path)
                    
                    # 定义函数签名
                    self.lib.umbrella_init.restype = UmbrellaResult
                    self.lib.umbrella_scan_file.restype = ScanResult
                    self.lib.umbrella_scan_file.argtypes = [ctypes.c_char_p]
                    self.lib.umbrella_scan_directory.restype = ScanResult
                    self.lib.umbrella_scan_directory.argtypes = [ctypes.c_char_p]
                    self.lib.umbrella_get_version.restype = ctypes.c_char_p
                    self.lib.umbrella_free_string.restype = None
                    self.lib.umbrella_free_string.argtypes = [ctypes.c_char_p]
                    self.lib.umbrella_cleanup.restype = UmbrellaResult
                    
                    print(f"✅ 成功加载 Umbrella 库: {dll_path}")
                    return True
                    
                except Exception as e:
                    print(f"❌ 加载库失败 {dll_path}: {e}")
                    continue
        
        print("❌ 无法找到 Umbrella 库")
        return False
    
    def initialize(self):
        """初始化 Umbrella 引擎"""
        if not self.lib:
            if not self.load_library():
                return False
        
        result = self.lib.umbrella_init()
        if result.success:
            self.initialized = True
            print("✅ Umbrella 引擎初始化成功")
            return True
        else:
            print(f"❌ 初始化失败，错误代码: {result.error_code}")
            return False
    
    def get_version(self):
        """获取版本信息"""
        if not self.initialized:
            return None
            
        version_ptr = self.lib.umbrella_get_version()
        if version_ptr:
            version = ctypes.string_at(version_ptr).decode('utf-8')
            self.lib.umbrella_free_string(version_ptr)
            return version
        return None
    
    def scan_current_scene(self):
        """扫描当前 Maya 场景"""
        if not self.initialized:
            print("❌ Umbrella 引擎未初始化")
            return None
        
        current_scene = cmds.file(query=True, sceneName=True)
        if not current_scene:
            print("ℹ️  当前没有打开的场景文件")
            return None
        
        print(f"🔍 扫描场景文件: {current_scene}")
        scene_bytes = current_scene.encode('utf-8')
        result = self.lib.umbrella_scan_file(scene_bytes)
        
        return {
            'file_path': current_scene,
            'threats_found': result.threats_found,
            'files_scanned': result.files_scanned,
            'scan_time_ms': result.scan_time_ms
        }
    
    def scan_maya_scripts_directory(self):
        """扫描 Maya 脚本目录"""
        if not self.initialized:
            print("❌ Umbrella 引擎未初始化")
            return None
        
        maya_app_dir = cmds.internalVar(userAppDir=True)
        scripts_dir = os.path.join(maya_app_dir, "scripts")
        
        if not os.path.exists(scripts_dir):
            print(f"⚠️  脚本目录不存在: {scripts_dir}")
            return None
        
        print(f"🔍 扫描脚本目录: {scripts_dir}")
        dir_bytes = scripts_dir.encode('utf-8')
        result = self.lib.umbrella_scan_directory(dir_bytes)
        
        return {
            'directory_path': scripts_dir,
            'threats_found': result.threats_found,
            'files_scanned': result.files_scanned,
            'scan_time_ms': result.scan_time_ms
        }
    
    def create_test_scene_with_threats(self):
        """创建一个包含威胁的测试场景"""
        # 创建新场景
        cmds.file(new=True, force=True)
        
        # 创建一些基本对象
        cube = cmds.polyCube(name="test_cube")[0]
        sphere = cmds.polySphere(name="test_sphere")[0]
        cmds.move(3, 0, 0, sphere)
        
        # 创建一个包含可疑代码的脚本节点
        suspicious_script = '''
import os
import subprocess
# 这是一个可疑的脚本
exec("print('potentially malicious code')")
eval("os.system('echo test')")
mel.eval("system(\\"dir\\");")
'''
        
        # 添加脚本节点
        script_node = cmds.scriptNode(
            name="suspicious_script_node",
            scriptType=2,  # Python
            beforeScript=suspicious_script
        )
        
        # 保存场景
        test_scene_path = os.path.join(tempfile.gettempdir(), "umbrella_test_scene_with_threats.ma")
        cmds.file(rename=test_scene_path)
        cmds.file(save=True, type="mayaAscii")
        
        print(f"📝 创建测试场景: {test_scene_path}")
        return test_scene_path
    
    def cleanup(self):
        """清理资源"""
        if self.lib and self.initialized:
            result = self.lib.umbrella_cleanup()
            if result.success:
                print("✅ Umbrella 引擎清理完成")
            else:
                print(f"⚠️  清理警告，错误代码: {result.error_code}")
            self.initialized = False

def demo_umbrella_integration():
    """演示 Umbrella Maya 集成"""
    print("=" * 60)
    print("🛡️  Umbrella Maya Plugin - 集成演示")
    print("=" * 60)
    
    # 创建集成实例
    umbrella = UmbrellaMayaIntegration()
    
    try:
        # 初始化
        if not umbrella.initialize():
            return
        
        # 显示版本信息
        version = umbrella.get_version()
        if version:
            print(f"📦 Umbrella 版本: {version}")
        
        # 演示1: 扫描当前场景
        print("\n" + "="*40)
        print("演示 1: 扫描当前场景")
        print("="*40)
        
        scene_result = umbrella.scan_current_scene()
        if scene_result:
            print(f"📊 扫描结果:")
            print(f"   文件: {scene_result['file_path']}")
            print(f"   威胁数量: {scene_result['threats_found']}")
            print(f"   扫描文件数: {scene_result['files_scanned']}")
            print(f"   扫描时间: {scene_result['scan_time_ms']}ms")
            
            if scene_result['threats_found'] > 0:
                print("⚠️  检测到威胁！请检查场景文件")
            else:
                print("✅ 当前场景安全")
        
        # 演示2: 创建并扫描包含威胁的场景
        print("\n" + "="*40)
        print("演示 2: 扫描包含威胁的测试场景")
        print("="*40)
        
        threat_scene_path = umbrella.create_test_scene_with_threats()
        
        # 扫描威胁场景
        threat_scene_bytes = threat_scene_path.encode('utf-8')
        threat_result = umbrella.lib.umbrella_scan_file(threat_scene_bytes)
        
        print(f"📊 威胁场景扫描结果:")
        print(f"   文件: {threat_scene_path}")
        print(f"   威胁数量: {threat_result.threats_found}")
        print(f"   扫描文件数: {threat_result.files_scanned}")
        print(f"   扫描时间: {threat_result.scan_time_ms}ms")
        
        if threat_result.threats_found > 0:
            print("⚠️  成功检测到威胁！")
        else:
            print("❌ 未能检测到威胁")
        
        # 演示3: 扫描脚本目录
        print("\n" + "="*40)
        print("演示 3: 扫描 Maya 脚本目录")
        print("="*40)
        
        scripts_result = umbrella.scan_maya_scripts_directory()
        if scripts_result:
            print(f"📊 脚本目录扫描结果:")
            print(f"   目录: {scripts_result['directory_path']}")
            print(f"   威胁数量: {scripts_result['threats_found']}")
            print(f"   扫描文件数: {scripts_result['files_scanned']}")
            print(f"   扫描时间: {scripts_result['scan_time_ms']}ms")
            
            if scripts_result['threats_found'] > 0:
                print("⚠️  脚本目录中检测到威胁！")
            else:
                print("✅ 脚本目录安全")
        
        print("\n" + "="*60)
        print("🎉 Umbrella Maya Plugin 集成演示完成！")
        print("🛡️  插件正常工作，可以保护您的 Maya 环境")
        print("="*60)
        
        # 清理临时文件
        if os.path.exists(threat_scene_path):
            os.unlink(threat_scene_path)
            print(f"🗑️  清理临时文件: {threat_scene_path}")
        
    except Exception as e:
        print(f"❌ 演示过程中发生错误: {e}")
    
    finally:
        # 清理资源
        umbrella.cleanup()

if __name__ == "__main__":
    # 检查是否在 Maya 环境中运行
    try:
        maya_version = cmds.about(version=True)
        print(f"🎬 检测到 Maya 版本: {maya_version}")
        demo_umbrella_integration()
    except:
        print("❌ 此脚本需要在 Maya Python 环境中运行")
        print("💡 请在 Maya 中执行: python(\"exec(open('demo_maya_integration.py').read())\")")
