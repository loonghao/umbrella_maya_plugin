/*
 * Umbrella Maya Plugin - Complete C++ Implementation
 * 
 * A comprehensive Maya C++ plugin that integrates with the Rust umbrella
 * antivirus library to provide real-time protection for Maya scenes.
 * 
 * Features:
 * - File and directory scanning
 * - Real-time scene monitoring
 * - Threat reporting and logging
 * - Integration with Maya UI
 */

#include <maya/MFnPlugin.h>
#include <maya/MPxCommand.h>
#include <maya/MArgList.h>
#include <maya/MGlobal.h>
#include <maya/MString.h>
#include <maya/MStatus.h>
#include <maya/MFileIO.h>
#include <maya/MSceneMessage.h>
#include <maya/MEventMessage.h>
#include <maya/MCallbackIdArray.h>
#include <maya/MFnDependencyNode.h>
#include <maya/MItDependencyNodes.h>
#include <maya/MPlug.h>

// Include the generated Rust bindings
#include "build/include/umbrella_maya_plugin.h"

#include <iostream>
#include <fstream>
#include <sstream>
#include <vector>
#include <string>

// Plugin information
static const char* kPluginName = "UmbrellaMayaPlugin";
static const char* kPluginVersion = "1.0.0";
static const char* kPluginVendor = "Umbrella Security Team";

// Command names
static const char* kScanFileCommand = "umbrellaScanFile";
static const char* kScanDirectoryCommand = "umbrellaScanDirectory";
static const char* kScanCurrentSceneCommand = "umbrellaScanScene";
static const char* kUmbrellaInfoCommand = "umbrellaInfo";
static const char* kUmbrellaStatusCommand = "umbrellaStatus";
static const char* kUmbrellaEnableCommand = "umbrellaEnable";
static const char* kUmbrellaDisableCommand = "umbrellaDisable";

// Global state
static bool g_umbrellaInitialized = false;
static bool g_realTimeProtectionEnabled = false;
static MCallbackIdArray g_callbackIds;

// Utility functions
namespace UmbrellaUtils {
    
    bool initializeUmbrella() {
        if (g_umbrellaInitialized) {
            return true;
        }
        
        UmbrellaResult result = umbrella_init();
        if (result.success) {
            g_umbrellaInitialized = true;
            MGlobal::displayInfo("Umbrella antivirus engine initialized successfully");
            return true;
        } else {
            MGlobal::displayError(MString("Failed to initialize Umbrella engine. Error code: ") + result.error_code);
            return false;
        }
    }
    
    void cleanupUmbrella() {
        if (g_umbrellaInitialized) {
            umbrella_cleanup();
            g_umbrellaInitialized = false;
        }
    }
    
    MString formatScanResult(const ScanResult& result, const MString& target) {
        MString msg;
        msg.format("Umbrella Scan Results for: ^1s\n", target);
        msg += MString("Files scanned: ") + result.files_scanned + "\n";
        msg += MString("Threats found: ") + result.threats_found + "\n";
        msg += MString("Scan time: ") + result.scan_time_ms + "ms\n";
        
        if (result.threats_found > 0) {
            msg += "⚠️ WARNING: Threats detected! Please review the scanned content.";
        } else if (result.threats_found == 0) {
            msg += "✅ No threats detected. Content appears safe.";
        } else {
            msg += "❌ Scan failed. Please check the file path and permissions.";
        }
        
        return msg;
    }
    
    void logThreatDetection(const MString& filePath, int threatCount) {
        if (threatCount > 0) {
            MString logMsg;
            logMsg.format("THREAT DETECTED: ^1s threats found in file: ^2s", 
                         MString() + threatCount, filePath);
            MGlobal::displayWarning(logMsg);
            
            // TODO: Write to log file
            std::cout << "[UMBRELLA] " << logMsg.asChar() << std::endl;
        }
    }
}

// Scene monitoring callbacks
void onSceneOpened(void* clientData) {
    if (!g_realTimeProtectionEnabled || !g_umbrellaInitialized) {
        return;
    }
    
    MString currentScene = MFileIO::currentFile();
    if (currentScene.length() > 0) {
        MGlobal::displayInfo("Umbrella: Scanning opened scene...");
        
        ScanResult result = umbrella_scan_file(currentScene.asChar());
        if (result.threats_found > 0) {
            UmbrellaUtils::logThreatDetection(currentScene, result.threats_found);
            MGlobal::displayWarning("Umbrella: Threats detected in opened scene!");
        }
    }
}

void onSceneSaved(void* clientData) {
    if (!g_realTimeProtectionEnabled || !g_umbrellaInitialized) {
        return;
    }

    MString currentScene = MFileIO::currentFile();
    if (currentScene.length() > 0) {
        ScanResult result = umbrella_scan_file(currentScene.asChar());
        if (result.threats_found > 0) {
            UmbrellaUtils::logThreatDetection(currentScene, result.threats_found);
        }
    }
}

//==============================================================================
// COMMAND IMPLEMENTATIONS
//==============================================================================

/**
 * Command: umbrellaScanFile
 * Scans a specific file for threats
 * Usage: umbrellaScanFile "path/to/file.ma"
 */
class UmbrellaScanFileCommand : public MPxCommand {
public:
    UmbrellaScanFileCommand() {}
    virtual ~UmbrellaScanFileCommand() {}

    static void* creator() {
        return new UmbrellaScanFileCommand();
    }

    virtual MStatus doIt(const MArgList& args) {
        if (!UmbrellaUtils::initializeUmbrella()) {
            return MS::kFailure;
        }

        // Get file path from arguments
        MString filePath;
        if (args.length() > 0) {
            MStatus status = args.get(0, filePath);
            if (status != MS::kSuccess) {
                MGlobal::displayError("Usage: umbrellaScanFile \"path/to/file\"");
                return MS::kFailure;
            }
        } else {
            // Default to current scene
            filePath = MFileIO::currentFile();
            if (filePath.length() == 0) {
                MGlobal::displayError("No file specified and no current scene open");
                return MS::kFailure;
            }
        }

        // Perform scan
        ScanResult result = umbrella_scan_file(filePath.asChar());

        // Display results
        MString resultMsg = UmbrellaUtils::formatScanResult(result, filePath);
        MGlobal::displayInfo(resultMsg);

        // Log threats if found
        UmbrellaUtils::logThreatDetection(filePath, result.threats_found);

        return MS::kSuccess;
    }
};

/**
 * Command: umbrellaScanDirectory
 * Scans a directory recursively for threats
 * Usage: umbrellaScanDirectory "path/to/directory"
 */
class UmbrellaScanDirectoryCommand : public MPxCommand {
public:
    UmbrellaScanDirectoryCommand() {}
    virtual ~UmbrellaScanDirectoryCommand() {}

    static void* creator() {
        return new UmbrellaScanDirectoryCommand();
    }

    virtual MStatus doIt(const MArgList& args) {
        if (!UmbrellaUtils::initializeUmbrella()) {
            return MS::kFailure;
        }

        // Get directory path from arguments
        MString dirPath;
        if (args.length() > 0) {
            MStatus status = args.get(0, dirPath);
            if (status != MS::kSuccess) {
                MGlobal::displayError("Usage: umbrellaScanDirectory \"path/to/directory\"");
                return MS::kFailure;
            }
        } else {
            MGlobal::displayError("Directory path required");
            return MS::kFailure;
        }

        MGlobal::displayInfo(MString("Scanning directory: ") + dirPath + " (this may take a while...)");

        // Perform directory scan
        ScanResult result = umbrella_scan_directory(dirPath.asChar());

        // Display results
        MString resultMsg = UmbrellaUtils::formatScanResult(result, dirPath);
        MGlobal::displayInfo(resultMsg);

        // Log threats if found
        if (result.threats_found > 0) {
            UmbrellaUtils::logThreatDetection(dirPath, result.threats_found);
        }

        return MS::kSuccess;
    }
};

/**
 * Command: umbrellaScanScene
 * Scans the current Maya scene for threats
 * Usage: umbrellaScanScene
 */
class UmbrellaScanSceneCommand : public MPxCommand {
public:
    UmbrellaScanSceneCommand() {}
    virtual ~UmbrellaScanSceneCommand() {}

    static void* creator() {
        return new UmbrellaScanSceneCommand();
    }

    virtual MStatus doIt(const MArgList& args) {
        if (!UmbrellaUtils::initializeUmbrella()) {
            return MS::kFailure;
        }

        MString currentScene = MFileIO::currentFile();
        if (currentScene.length() == 0) {
            MGlobal::displayError("No scene is currently open");
            return MS::kFailure;
        }

        MGlobal::displayInfo("Scanning current Maya scene...");

        // Perform scan
        ScanResult result = umbrella_scan_file(currentScene.asChar());

        // Display results
        MString resultMsg = UmbrellaUtils::formatScanResult(result, "Current Scene");
        MGlobal::displayInfo(resultMsg);

        // Log threats if found
        UmbrellaUtils::logThreatDetection(currentScene, result.threats_found);

        return MS::kSuccess;
    }
};

/**
 * Command: umbrellaInfo
 * Displays information about the Umbrella plugin
 * Usage: umbrellaInfo
 */
class UmbrellaInfoCommand : public MPxCommand {
public:
    UmbrellaInfoCommand() {}
    virtual ~UmbrellaInfoCommand() {}

    static void* creator() {
        return new UmbrellaInfoCommand();
    }

    virtual MStatus doIt(const MArgList& args) {
        MString info;
        info += "=== Umbrella Maya Plugin Information ===\n";
        info += MString("Plugin Name: ") + kPluginName + "\n";
        info += MString("Version: ") + kPluginVersion + "\n";
        info += MString("Vendor: ") + kPluginVendor + "\n";

        // Get Rust library version
        if (UmbrellaUtils::initializeUmbrella()) {
            char* rustVersion = umbrella_get_version();
            if (rustVersion != nullptr) {
                info += MString("Rust Library Version: ") + rustVersion + "\n";
                umbrella_free_string(rustVersion);
            }
        }

        info += MString("Engine Status: ") + (g_umbrellaInitialized ? "Initialized" : "Not Initialized") + "\n";
        info += MString("Real-time Protection: ") + (g_realTimeProtectionEnabled ? "Enabled" : "Disabled") + "\n";
        info += "\nAvailable Commands:\n";
        info += "  umbrellaScanFile [path]     - Scan a specific file\n";
        info += "  umbrellaScanDirectory path  - Scan a directory\n";
        info += "  umbrellaScanScene          - Scan current scene\n";
        info += "  umbrellaStatus             - Show protection status\n";
        info += "  umbrellaEnable             - Enable real-time protection\n";
        info += "  umbrellaDisable            - Disable real-time protection\n";
        info += "  umbrellaInfo               - Show this information\n";

        MGlobal::displayInfo(info);
        return MS::kSuccess;
    }
};

/**
 * Command: umbrellaStatus
 * Shows the current status of Umbrella protection
 * Usage: umbrellaStatus
 */
class UmbrellaStatusCommand : public MPxCommand {
public:
    UmbrellaStatusCommand() {}
    virtual ~UmbrellaStatusCommand() {}

    static void* creator() {
        return new UmbrellaStatusCommand();
    }

    virtual MStatus doIt(const MArgList& args) {
        MString status;
        status += "=== Umbrella Protection Status ===\n";
        status += MString("Engine: ") + (g_umbrellaInitialized ? "✅ Running" : "❌ Stopped") + "\n";
        status += MString("Real-time Protection: ") + (g_realTimeProtectionEnabled ? "✅ Enabled" : "❌ Disabled") + "\n";
        status += MString("Active Callbacks: ") + g_callbackIds.length() + "\n";

        if (g_umbrellaInitialized) {
            status += "🛡️ Your Maya environment is protected by Umbrella";
        } else {
            status += "⚠️ Umbrella protection is not active";
        }

        MGlobal::displayInfo(status);
        return MS::kSuccess;
    }
};

/**
 * Command: umbrellaEnable
 * Enables real-time protection
 * Usage: umbrellaEnable
 */
class UmbrellaEnableCommand : public MPxCommand {
public:
    UmbrellaEnableCommand() {}
    virtual ~UmbrellaEnableCommand() {}

    static void* creator() {
        return new UmbrellaEnableCommand();
    }

    virtual MStatus doIt(const MArgList& args) {
        if (!UmbrellaUtils::initializeUmbrella()) {
            return MS::kFailure;
        }

        if (g_realTimeProtectionEnabled) {
            MGlobal::displayInfo("Real-time protection is already enabled");
            return MS::kSuccess;
        }

        // Register scene callbacks
        MCallbackId openCallbackId = MSceneMessage::addCallback(
            MSceneMessage::kAfterOpen, onSceneOpened, nullptr);
        MCallbackId saveCallbackId = MSceneMessage::addCallback(
            MSceneMessage::kAfterSave, onSceneSaved, nullptr);

        if (openCallbackId != 0 && saveCallbackId != 0) {
            g_callbackIds.append(openCallbackId);
            g_callbackIds.append(saveCallbackId);
            g_realTimeProtectionEnabled = true;

            MGlobal::displayInfo("✅ Umbrella real-time protection enabled");
            MGlobal::displayInfo("Maya scenes will be automatically scanned when opened or saved");
        } else {
            MGlobal::displayError("Failed to register scene callbacks");
            return MS::kFailure;
        }

        return MS::kSuccess;
    }
};

/**
 * Command: umbrellaDisable
 * Disables real-time protection
 * Usage: umbrellaDisable
 */
class UmbrellaDisableCommand : public MPxCommand {
public:
    UmbrellaDisableCommand() {}
    virtual ~UmbrellaDisableCommand() {}

    static void* creator() {
        return new UmbrellaDisableCommand();
    }

    virtual MStatus doIt(const MArgList& args) {
        if (!g_realTimeProtectionEnabled) {
            MGlobal::displayInfo("Real-time protection is already disabled");
            return MS::kSuccess;
        }

        // Remove all callbacks
        for (unsigned int i = 0; i < g_callbackIds.length(); i++) {
            MMessage::removeCallback(g_callbackIds[i]);
        }
        g_callbackIds.clear();
        g_realTimeProtectionEnabled = false;

        MGlobal::displayInfo("❌ Umbrella real-time protection disabled");
        return MS::kSuccess;
    }
};

//==============================================================================
// PLUGIN INITIALIZATION AND CLEANUP
//==============================================================================

/**
 * Plugin initialization function
 */
MStatus initializePlugin(MObject obj) {
    MStatus status;
    MFnPlugin plugin(obj, kPluginVendor, kPluginVersion, "Any");

    // Register all commands
    status = plugin.registerCommand(kScanFileCommand, UmbrellaScanFileCommand::creator);
    if (!status) {
        status.perror("Failed to register umbrellaScanFile command");
        return status;
    }

    status = plugin.registerCommand(kScanDirectoryCommand, UmbrellaScanDirectoryCommand::creator);
    if (!status) {
        status.perror("Failed to register umbrellaScanDirectory command");
        return status;
    }

    status = plugin.registerCommand(kScanCurrentSceneCommand, UmbrellaScanSceneCommand::creator);
    if (!status) {
        status.perror("Failed to register umbrellaScanScene command");
        return status;
    }

    status = plugin.registerCommand(kUmbrellaInfoCommand, UmbrellaInfoCommand::creator);
    if (!status) {
        status.perror("Failed to register umbrellaInfo command");
        return status;
    }

    status = plugin.registerCommand(kUmbrellaStatusCommand, UmbrellaStatusCommand::creator);
    if (!status) {
        status.perror("Failed to register umbrellaStatus command");
        return status;
    }

    status = plugin.registerCommand(kUmbrellaEnableCommand, UmbrellaEnableCommand::creator);
    if (!status) {
        status.perror("Failed to register umbrellaEnable command");
        return status;
    }

    status = plugin.registerCommand(kUmbrellaDisableCommand, UmbrellaDisableCommand::creator);
    if (!status) {
        status.perror("Failed to register umbrellaDisable command");
        return status;
    }

    // Initialize Umbrella engine
    if (UmbrellaUtils::initializeUmbrella()) {
        MGlobal::displayInfo("🛡️ Umbrella Maya Plugin loaded successfully!");
        MGlobal::displayInfo("Type 'umbrellaInfo' for available commands");

        // Get and display version info
        char* version = umbrella_get_version();
        if (version != nullptr) {
            MString versionMsg = MString("Rust library version: ") + version;
            MGlobal::displayInfo(versionMsg);
            umbrella_free_string(version);
        }
    } else {
        MGlobal::displayWarning("Umbrella engine initialization failed - some features may not work");
    }

    return status;
}

/**
 * Plugin cleanup function
 */
MStatus uninitializePlugin(MObject obj) {
    MStatus status;
    MFnPlugin plugin(obj);

    // Disable real-time protection first
    if (g_realTimeProtectionEnabled) {
        for (unsigned int i = 0; i < g_callbackIds.length(); i++) {
            MMessage::removeCallback(g_callbackIds[i]);
        }
        g_callbackIds.clear();
        g_realTimeProtectionEnabled = false;
    }

    // Deregister all commands
    status = plugin.deregisterCommand(kScanFileCommand);
    if (!status) {
        status.perror("Failed to deregister umbrellaScanFile command");
    }

    status = plugin.deregisterCommand(kScanDirectoryCommand);
    if (!status) {
        status.perror("Failed to deregister umbrellaScanDirectory command");
    }

    status = plugin.deregisterCommand(kScanCurrentSceneCommand);
    if (!status) {
        status.perror("Failed to deregister umbrellaScanScene command");
    }

    status = plugin.deregisterCommand(kUmbrellaInfoCommand);
    if (!status) {
        status.perror("Failed to deregister umbrellaInfo command");
    }

    status = plugin.deregisterCommand(kUmbrellaStatusCommand);
    if (!status) {
        status.perror("Failed to deregister umbrellaStatus command");
    }

    status = plugin.deregisterCommand(kUmbrellaEnableCommand);
    if (!status) {
        status.perror("Failed to deregister umbrellaEnable command");
    }

    status = plugin.deregisterCommand(kUmbrellaDisableCommand);
    if (!status) {
        status.perror("Failed to deregister umbrellaDisable command");
    }

    // Cleanup Umbrella engine
    UmbrellaUtils::cleanupUmbrella();

    MGlobal::displayInfo("Umbrella Maya Plugin unloaded successfully");
    return MS::kSuccess;
}
