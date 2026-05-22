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
#include <maya/MFileObject.h>
#include <maya/MSceneMessage.h>
#include <maya/MEventMessage.h>
#include <maya/MMessage.h>
#include <maya/MCallbackIdArray.h>
#include <maya/MFnDependencyNode.h>
#include <maya/MItDependencyNodes.h>
#include <maya/MPlug.h>
#include <maya/MStringArray.h>
#include <maya/MSelectionList.h>

// Include the generated Rust bindings
#include "umbrella_maya_plugin.h"

#include <iostream>
#include <fstream>
#include <sstream>
#include <vector>
#include <string>
#include <cwchar>
#include <cstdlib>
#include <cctype>
#include <algorithm>
#include <filesystem>

#ifdef _WIN32
#include <windows.h>
#endif

// Plugin information
static const char* kPluginName = "umbrella_maya";
static const char* kPluginVersion = "1.0.0";
static const char* kPluginVendor = "Umbrella Security Team";

// Command names
static const char* kScanFileCommand = "umbrellaScanFile";
static const char* kScanDirectoryCommand = "umbrellaScanDirectory";
static const char* kScanCurrentSceneCommand = "umbrellaScanScene";
static const char* kCleanFileCommand = "umbrellaCleanFile";
static const char* kCleanDirectoryCommand = "umbrellaCleanDirectory";
static const char* kFixSceneCommand = "umbrellaFixScene";
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

#ifdef _WIN32
    static HMODULE g_rustDllHandle = nullptr;

    bool ensureRustRuntimeLoaded() {
        if (g_rustDllHandle != nullptr) {
            return true;
        }

        HMODULE pluginModule = nullptr;
        if (!GetModuleHandleExW(
                GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                reinterpret_cast<LPCWSTR>(&ensureRustRuntimeLoaded),
                &pluginModule)) {
            MGlobal::displayError("Umbrella: failed to locate plugin module for DLL dependency loading");
            return false;
        }

        wchar_t pluginPath[MAX_PATH] = {};
        DWORD length = GetModuleFileNameW(pluginModule, pluginPath, MAX_PATH);
        if (length == 0 || length >= MAX_PATH) {
            MGlobal::displayError("Umbrella: failed to resolve plugin module path");
            return false;
        }

        wchar_t* lastBackslash = std::wcsrchr(pluginPath, L'\\');
        wchar_t* lastSlash = std::wcsrchr(pluginPath, L'/');
        wchar_t* separator = lastBackslash > lastSlash ? lastBackslash : lastSlash;
        if (separator == nullptr) {
            MGlobal::displayError("Umbrella: failed to resolve plugin directory");
            return false;
        }

        *(separator + 1) = L'\0';
        std::wstring rustDllPath = std::wstring(pluginPath) + L"umbrella_maya_plugin.dll";
        SetDllDirectoryW(pluginPath);

        g_rustDllHandle = LoadLibraryW(rustDllPath.c_str());
        if (g_rustDllHandle == nullptr) {
            MGlobal::displayError(MString("Umbrella: failed to load umbrella_maya_plugin.dll. Windows error: ") + static_cast<int>(GetLastError()));
            return false;
        }

        return true;
    }
#else
    bool ensureRustRuntimeLoaded() {
        return true;
    }
#endif
    
    bool initializeUmbrella() {
        if (g_umbrellaInitialized) {
            return true;
        }

        if (!ensureRustRuntimeLoaded()) {
            return false;
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
            msg += "WARNING: Threats detected. Please review the scanned content.";
        } else if (result.threats_found == 0) {
            msg += "No threats detected. Content appears safe.";
        } else {
            msg += "Scan failed. Please check the file path and permissions.";
        }
        
        return msg;
    }

    MString formatCleanResult(const CleanFFIResult& result, const MString& target) {
        MString msg;
        msg.format("Umbrella Clean Results for: ^1s\n", target);
        msg += MString("Files cleaned: ") + result.files_cleaned + "\n";
        msg += MString("Files deleted: ") + result.files_deleted + "\n";
        msg += MString("Files failed: ") + result.files_failed + "\n";
        msg += MString("Threat signatures removed: ") + result.threats_removed + "\n";
        msg += MString("Clean time: ") + result.scan_time_ms + "ms\n";
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

    bool envTrue(const char* name) {
        const char* value = std::getenv(name);
        if (value == nullptr) {
            return false;
        }
        std::string text(value);
        for (char& ch : text) {
            ch = static_cast<char>(std::tolower(static_cast<unsigned char>(ch)));
        }
        return text == "1" || text == "true" || text == "yes" || text == "on";
    }

    bool hookDisabled(const char* hookName) {
        if (envTrue("MAYA_UMBRELLA_DISABLE_ALL_HOOKS")) {
            return true;
        }
        const char* disabled = std::getenv("MAYA_UMBRELLA_DISABLE_HOOKS");
        if (disabled == nullptr) {
            return false;
        }
        std::string list(disabled);
        std::string hook(hookName);
        size_t start = 0;
        while (start <= list.size()) {
            size_t comma = list.find(',', start);
            std::string item = list.substr(start, comma == std::string::npos ? std::string::npos : comma - start);
            size_t first = item.find_first_not_of(" \t");
            size_t last = item.find_last_not_of(" \t");
            if (first != std::string::npos && item.substr(first, last - first + 1) == hook) {
                return true;
            }
            if (comma == std::string::npos) {
                break;
            }
            start = comma + 1;
        }
        return false;
    }

    MString quote(const MString& value) {
        std::string input(value.asChar());
        std::string escaped;
        escaped.reserve(input.size() + 2);
        escaped.push_back('"');
        for (char ch : input) {
            if (ch == '\\' || ch == '"') {
                escaped.push_back('\\');
            }
            escaped.push_back(ch);
        }
        escaped.push_back('"');
        return MString(escaped.c_str());
    }

    void executeQuiet(const MString& command) {
        MStatus status = MGlobal::executeCommand(command, false, false);
        if (!status) {
            MGlobal::displayWarning(MString("Umbrella command failed: ") + command);
        }
    }

    bool isReferencedNode(const MFnDependencyNode& node) {
        return node.isFromReferencedFile();
    }

    MString plugStringValue(MFnDependencyNode& node, const char* attr) {
        MStatus status;
        MPlug plug = node.findPlug(attr, false, &status);
        if (!status) {
            return MString();
        }
        MString value;
        plug.getValue(value);
        return value;
    }

    bool containsThreat(const MString& content) {
        if (content.length() == 0) {
            return false;
        }
        return umbrella_scan_content(content.asChar()) > 0;
    }

    void clearScriptNode(const MString& nodeName) {
        executeQuiet(MString("setAttr ") + quote(nodeName + ".before") + " -type \"string\" \"\"");
        executeQuiet(MString("setAttr ") + quote(nodeName + ".after") + " -type \"string\" \"\"");
        executeQuiet(MString("setAttr ") + quote(nodeName + ".scriptType") + " 0");
    }

    int fixInfectedScriptNodes() {
        int fixed = 0;
        MItDependencyNodes it(MFn::kScript);
        for (; !it.isDone(); it.next()) {
            MObject nodeObject = it.thisNode();
            MFnDependencyNode node(nodeObject);
            MString nodeName = node.name();
            std::string name(nodeName.asChar());
            MString before = plugStringValue(node, "before");
            MString after = plugStringValue(node, "after");
            MString notes = plugStringValue(node, "notes");

            bool infected =
                name.find("_gene") != std::string::npos ||
                name == "maya_secure_system_scriptNode" ||
                name == "uifiguration" ||
                name.find("codeExtractor") != std::string::npos ||
                name.find("codeChunk") != std::string::npos ||
                containsThreat(before) ||
                containsThreat(after) ||
                containsThreat(notes);

            if (!infected) {
                continue;
            }

            if (isReferencedNode(node)) {
                clearScriptNode(nodeName);
            } else {
                executeQuiet(MString("lockNode -lock off ") + quote(nodeName));
                executeQuiet(MString("delete ") + quote(nodeName));
            }
            fixed++;
        }
        return fixed;
    }

    int killInfectedScriptJobs() {
        MStringArray jobs;
        MGlobal::executeCommand("scriptJob -listJobs", jobs, false, false);
        int killed = 0;
        for (unsigned int i = 0; i < jobs.length(); ++i) {
            std::string job(jobs[i].asChar());
            if (job.find("leukocyte") == std::string::npos && job.find("execute") == std::string::npos) {
                continue;
            }
            size_t colon = job.find(':');
            if (colon == std::string::npos) {
                continue;
            }
            std::string id = job.substr(0, colon);
            if (!id.empty() && std::all_of(id.begin(), id.end(), [](char ch) { return std::isdigit(static_cast<unsigned char>(ch)); })) {
                executeQuiet(MString("scriptJob -kill ") + id.c_str() + " -force");
                killed++;
            }
        }
        return killed;
    }

    int deleteUnknownPluginNodes() {
        int deleted = 0;
        MStringArray unknownNodes;
        MGlobal::executeCommand("ls -type unknown", unknownNodes, false, false);
        for (unsigned int i = 0; i < unknownNodes.length(); ++i) {
            MString nodeName = unknownNodes[i];
            MSelectionList selection;
            MStatus status = selection.add(nodeName);
            if (status) {
                MObject object;
                selection.getDependNode(0, object);
                MFnDependencyNode node(object);
                if (isReferencedNode(node)) {
                    continue;
                }
            }
            executeQuiet(MString("lockNode -lock off ") + quote(nodeName));
            executeQuiet(MString("delete ") + quote(nodeName));
            deleted++;
        }

        MStringArray unknownPlugins;
        MGlobal::executeCommand("unknownPlugin -q -l", unknownPlugins, false, false);
        for (unsigned int i = 0; i < unknownPlugins.length(); ++i) {
            executeQuiet(MString("unknownPlugin -remove ") + quote(unknownPlugins[i]));
        }
        return deleted;
    }

    int deleteTurtleNodes() {
        const char* plugins[] = {"Turtle.mll", "mayatomr.mll"};
        for (const char* plugin : plugins) {
            executeQuiet(MString("if (`pluginInfo -q -loaded ") + quote(plugin) + "`) unloadPlugin -f " + quote(plugin));
        }

        const char* nodes[] = {
            "TurtleRenderOptions",
            "TurtleUIOptions",
            "TurtleBakeLayerManager",
            "TurtleDefaultBakeLayer",
        };
        int deleted = 0;
        for (const char* node : nodes) {
            executeQuiet(MString("if (`objExists ") + quote(node) + "`) { lockNode -lock off " + quote(node) + "; delete " + quote(node) + "; }");
            deleted++;
        }
        if (MGlobal::mayaState() == MGlobal::kInteractive) {
            executeQuiet("global string $gShelfTopLevel; string $shelves[] = `tabLayout -q -ca $gShelfTopLevel`; if (stringArrayContains(\"TURTLE\", $shelves)) deleteUI -layout \"TURTLE\";");
        }
        return deleted;
    }

    void fixModelPanels() {
        executeQuiet("string $panels[] = `getPanel -type modelPanel`; for ($panel in $panels) { string $cb = `modelEditor -q -editorChanged $panel`; if ($cb == \"CgAbBlastPanelOptChangeCallback\") modelEditor -e -editorChanged \"\" $panel; }");
    }

    void fixOnModelChange3dc() {
        executeQuiet("global proc onModelChange3dc(string $a){}");
        executeQuiet("if (`objExists \"fixCgAbBlastPanelOptChangeCallback\"`) delete \"fixCgAbBlastPanelOptChangeCallback\"");
        executeQuiet("global proc CgAbBlastPanelOptChangeCallback(string $i){}");
    }

    void removeRenameTempFiles() {
        MString userScriptDir;
        MGlobal::executeCommand("internalVar -userScriptDir", userScriptDir, false, false);
        if (userScriptDir.length() == 0) {
            return;
        }
        std::error_code ec;
        std::filesystem::path root(userScriptDir.asChar());
        if (!std::filesystem::exists(root, ec)) {
            return;
        }
        for (const auto& entry : std::filesystem::directory_iterator(root, ec)) {
            if (ec) {
                break;
            }
            std::string name = entry.path().filename().string();
            if (name.rfind("._", 0) == 0) {
                std::filesystem::remove(entry.path(), ec);
            }
        }
    }

    int runSceneFixHooks() {
        int fixed = 0;
        if (!hookDisabled("delete_turtle")) {
            fixed += deleteTurtleNodes();
        }
        if (!hookDisabled("delete_unknown_plugin_node")) {
            fixed += deleteUnknownPluginNodes();
        }
        if (!hookDisabled("fix_model_panel")) {
            fixModelPanels();
        }
        if (!hookDisabled("fix_on_model_change_3dc")) {
            fixOnModelChange3dc();
        }
        removeRenameTempFiles();
        fixed += fixInfectedScriptNodes();
        fixed += killInfectedScriptJobs();
        return fixed;
    }

    MString fileObjectPath(MFileObject& file) {
        MString resolved = file.resolvedFullName();
        if (resolved.length() > 0) {
            return resolved;
        }
        return file.rawFullName();
    }
}

// Scene monitoring callbacks
void onSceneBeforeOpenCheck(bool* retCode, MFileObject& file, void* clientData) {
    if (retCode == nullptr) {
        return;
    }
    *retCode = true;

    if (!g_realTimeProtectionEnabled || !g_umbrellaInitialized) {
        return;
    }

    MString scenePath = UmbrellaUtils::fileObjectPath(file);
    if (scenePath.length() == 0) {
        return;
    }

    ScanResult result = umbrella_scan_file(scenePath.asChar());
    if (result.threats_found <= 0) {
        return;
    }

    UmbrellaUtils::logThreatDetection(scenePath, result.threats_found);
    MGlobal::displayWarning("Umbrella: Threats detected before scene open. Cleaning scene before Maya executes script nodes...");

    CleanFFIResult cleanResult = umbrella_clean_file(scenePath.asChar());
    if (cleanResult.files_failed > 0) {
        MGlobal::displayError("Umbrella: Scene cleanup failed. Blocking open to avoid executing malicious scene content.");
        *retCode = false;
        return;
    }

    if (cleanResult.files_deleted > 0) {
        MGlobal::displayError("Umbrella: Scene was removed during cleanup. Blocking open because there is no sanitized scene to load.");
        *retCode = false;
        return;
    }

    MGlobal::displayInfo(MString("Umbrella: Scene sanitized before open. Threat signatures removed: ") + cleanResult.threats_removed);
}

void onSceneOpened(void* clientData) {
    if (!g_realTimeProtectionEnabled || !g_umbrellaInitialized) {
        return;
    }
    
    MString currentScene = MFileIO::currentFile();
    if (currentScene.length() > 0) {
        MGlobal::displayInfo("Umbrella: Scanning opened scene...");
        UmbrellaUtils::runSceneFixHooks();
        
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
        UmbrellaUtils::runSceneFixHooks();
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
 * Command: umbrellaCleanFile
 * Cleans a specific file by removing known file-level signatures.
 * Usage: umbrellaCleanFile "path/to/file"
 */
class UmbrellaCleanFileCommand : public MPxCommand {
public:
    UmbrellaCleanFileCommand() {}
    virtual ~UmbrellaCleanFileCommand() {}

    static void* creator() {
        return new UmbrellaCleanFileCommand();
    }

    virtual MStatus doIt(const MArgList& args) {
        if (!UmbrellaUtils::initializeUmbrella()) {
            return MS::kFailure;
        }

        MString filePath;
        if (args.length() > 0) {
            MStatus status = args.get(0, filePath);
            if (status != MS::kSuccess) {
                MGlobal::displayError("Usage: umbrellaCleanFile \"path/to/file\"");
                return MS::kFailure;
            }
        } else {
            filePath = MFileIO::currentFile();
            if (filePath.length() == 0) {
                MGlobal::displayError("No file specified and no current scene open");
                return MS::kFailure;
            }
        }

        CleanFFIResult result = umbrella_clean_file(filePath.asChar());
        MGlobal::displayInfo(UmbrellaUtils::formatCleanResult(result, filePath));
        return result.files_failed == 0 ? MS::kSuccess : MS::kFailure;
    }
};

/**
 * Command: umbrellaCleanDirectory
 * Cleans supported files under a directory.
 * Usage: umbrellaCleanDirectory "path/to/directory"
 */
class UmbrellaCleanDirectoryCommand : public MPxCommand {
public:
    UmbrellaCleanDirectoryCommand() {}
    virtual ~UmbrellaCleanDirectoryCommand() {}

    static void* creator() {
        return new UmbrellaCleanDirectoryCommand();
    }

    virtual MStatus doIt(const MArgList& args) {
        if (!UmbrellaUtils::initializeUmbrella()) {
            return MS::kFailure;
        }

        MString dirPath;
        if (args.length() > 0) {
            MStatus status = args.get(0, dirPath);
            if (status != MS::kSuccess) {
                MGlobal::displayError("Usage: umbrellaCleanDirectory \"path/to/directory\"");
                return MS::kFailure;
            }
        } else {
            MGlobal::displayError("Directory path required");
            return MS::kFailure;
        }

        CleanFFIResult result = umbrella_clean_directory(dirPath.asChar());
        MGlobal::displayInfo(UmbrellaUtils::formatCleanResult(result, dirPath));
        return result.files_failed == 0 ? MS::kSuccess : MS::kFailure;
    }
};

/**
 * Command: umbrellaFixScene
 * Runs the scene-level cleanup hooks ported from maya_umbrella.
 * Usage: umbrellaFixScene
 */
class UmbrellaFixSceneCommand : public MPxCommand {
public:
    UmbrellaFixSceneCommand() {}
    virtual ~UmbrellaFixSceneCommand() {}

    static void* creator() {
        return new UmbrellaFixSceneCommand();
    }

    virtual MStatus doIt(const MArgList& args) {
        if (!UmbrellaUtils::initializeUmbrella()) {
            return MS::kFailure;
        }

        int fixed = UmbrellaUtils::runSceneFixHooks();
        MGlobal::displayInfo(MString("Umbrella scene cleanup complete. Items handled: ") + fixed);
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
        info += "  umbrellaCleanFile [path]    - Clean a specific file\n";
        info += "  umbrellaCleanDirectory path - Clean a directory\n";
        info += "  umbrellaFixScene           - Run scene cleanup hooks\n";
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
        status += MString("Engine: ") + (g_umbrellaInitialized ? "Running" : "Stopped") + "\n";
        status += MString("Real-time Protection: ") + (g_realTimeProtectionEnabled ? "Enabled" : "Disabled") + "\n";
        status += MString("Active Callbacks: ") + g_callbackIds.length() + "\n";

        if (g_umbrellaInitialized) {
            status += "Your Maya environment is protected by Umbrella";
        } else {
            status += "Umbrella protection is not active";
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
        MCallbackId beforeOpenCallbackId = MSceneMessage::addCheckFileCallback(
            MSceneMessage::kBeforeOpenCheck, onSceneBeforeOpenCheck, nullptr);
        MCallbackId openCallbackId = MSceneMessage::addCallback(
            MSceneMessage::kAfterOpen, onSceneOpened, nullptr);
        MCallbackId saveCallbackId = MSceneMessage::addCallback(
            MSceneMessage::kAfterSave, onSceneSaved, nullptr);

        if (beforeOpenCallbackId != 0 && openCallbackId != 0 && saveCallbackId != 0) {
            g_callbackIds.append(beforeOpenCallbackId);
            g_callbackIds.append(openCallbackId);
            g_callbackIds.append(saveCallbackId);
            g_realTimeProtectionEnabled = true;

            MGlobal::displayInfo("Umbrella real-time protection enabled");
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

        MGlobal::displayInfo("Umbrella real-time protection disabled");
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

    std::vector<const char*> registeredCommands;
    auto registerCommand = [&](const char* name, MCreatorFunction creator) -> MStatus {
        MStatus registerStatus = plugin.registerCommand(name, creator);
        if (!registerStatus) {
            registerStatus.perror(MString("Failed to register command: ") + name);
            for (auto it = registeredCommands.rbegin(); it != registeredCommands.rend(); ++it) {
                plugin.deregisterCommand(*it);
            }
            return registerStatus;
        }
        registeredCommands.push_back(name);
        return MS::kSuccess;
    };

    status = registerCommand(kScanFileCommand, UmbrellaScanFileCommand::creator);
    if (!status) return status;
    status = registerCommand(kScanDirectoryCommand, UmbrellaScanDirectoryCommand::creator);
    if (!status) return status;
    status = registerCommand(kScanCurrentSceneCommand, UmbrellaScanSceneCommand::creator);
    if (!status) return status;
    status = registerCommand(kCleanFileCommand, UmbrellaCleanFileCommand::creator);
    if (!status) return status;
    status = registerCommand(kCleanDirectoryCommand, UmbrellaCleanDirectoryCommand::creator);
    if (!status) return status;
    status = registerCommand(kFixSceneCommand, UmbrellaFixSceneCommand::creator);
    if (!status) return status;
    status = registerCommand(kUmbrellaInfoCommand, UmbrellaInfoCommand::creator);
    if (!status) return status;
    status = registerCommand(kUmbrellaStatusCommand, UmbrellaStatusCommand::creator);
    if (!status) return status;
    status = registerCommand(kUmbrellaEnableCommand, UmbrellaEnableCommand::creator);
    if (!status) return status;
    status = registerCommand(kUmbrellaDisableCommand, UmbrellaDisableCommand::creator);
    if (!status) return status;

    // Initialize Umbrella engine
    if (UmbrellaUtils::initializeUmbrella()) {
        MGlobal::displayInfo("Umbrella Maya Plugin loaded successfully");
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

    status = plugin.deregisterCommand(kCleanFileCommand);
    if (!status) {
        status.perror("Failed to deregister umbrellaCleanFile command");
    }

    status = plugin.deregisterCommand(kCleanDirectoryCommand);
    if (!status) {
        status.perror("Failed to deregister umbrellaCleanDirectory command");
    }

    status = plugin.deregisterCommand(kFixSceneCommand);
    if (!status) {
        status.perror("Failed to deregister umbrellaFixScene command");
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
