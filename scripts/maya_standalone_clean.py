"""Conservative copy-only native cleanup. Run via umbrella-maya, with real mayapy.

A production environment manifest is mandatory. This does not provide a sandbox
for hostile scenes. References and unreadable/unknown scene content fail closed.
"""
from __future__ import print_function

import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile


def digest(path):
    checksum = hashlib.sha256()
    with open(path, "rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            checksum.update(block)
    return checksum.hexdigest()


def verify_file(entry):
    path = entry["path"]
    if not os.path.isabs(path) or not os.path.isfile(path):
        raise RuntimeError("Production dependency is missing or not absolute: " + path)
    if digest(path) != entry["sha256"]:
        raise RuntimeError("Production dependency hash mismatch: " + path)


def load_profile(path, plugin, expected_year):
    with open(path, "r") as stream:
        profile = json.load(stream)
    required = ("maya_version", "api_version", "platform", "mayapy_sha256", "plugins", "dependencies")
    if profile.get("schema_version") != 1 or any(key not in profile for key in required):
        raise RuntimeError("Incomplete environment profile; see docs/binary-scene-safety.md")
    if profile["maya_version"].split(".")[0] != expected_year:
        raise RuntimeError("Environment profile does not match --maya-version")
    if profile["platform"] != sys.platform or digest(sys.executable) != profile["mayapy_sha256"]:
        raise RuntimeError("mayapy executable or platform does not match the production profile")
    if not profile["plugins"] or not profile["dependencies"]:
        raise RuntimeError("Profile must list production plugins and runtime dependencies")
    plugins = profile["plugins"]
    if len(set(entry["name"] for entry in plugins)) != len(plugins):
        raise RuntimeError("Duplicate plugin names in environment profile")
    if not any(os.path.realpath(entry["path"]) == os.path.realpath(plugin) for entry in plugins):
        raise RuntimeError("--plugin is absent from the production environment profile")
    for entry in plugins + profile["dependencies"]:
        verify_file(entry)
    return profile


def verify_maya(cmds, profile):
    for dependency in profile["dependencies"]:
        verify_file(dependency)
    if str(cmds.about(version=True)) != profile["maya_version"]:
        raise RuntimeError("Running Maya version differs from the production profile")
    if int(cmds.about(apiVersion=True)) != profile["api_version"]:
        raise RuntimeError("Running Maya API version differs from the production profile")
    expected = {entry["name"]: entry for entry in profile["plugins"]}
    loaded = set(cmds.pluginInfo(query=True, listPlugins=True) or [])
    if loaded != set(expected):
        raise RuntimeError("Loaded plugin inventory differs from production: missing=%r unexpected=%r" %
                           (sorted(set(expected) - loaded), sorted(loaded - set(expected))))
    for name, entry in expected.items():
        actual = cmds.pluginInfo(name, query=True, path=True)
        if os.path.realpath(actual) != os.path.realpath(entry["path"]):
            raise RuntimeError("Loaded plugin path mismatch: " + name)
        if str(cmds.pluginInfo(name, query=True, version=True)) != entry["version"]:
            raise RuntimeError("Loaded plugin version mismatch: " + name)
        verify_file(entry)


def verify_scene(cmds):
    if cmds.file(query=True, errorStatus=True):
        raise RuntimeError("Maya reported a scene read/write error; refusing to save")
    references = cmds.file(query=True, reference=True) or []
    if references:
        raise RuntimeError("Referenced scenes are not supported by this safety stage; references are not classified as malware. Original preserved: %r" % references)
    unknown = []
    for node_type in ("unknown", "unknownDag", "unknownTransform"):
        unknown.extend(cmds.ls(type=node_type) or [])
    if unknown or (cmds.unknownPlugin(query=True, list=True) or []):
        raise RuntimeError("Unknown nodes/plugins may indicate missing production dependencies; refusing lossy save: %r" % unknown)


def scan_output(cli, path):
    process = subprocess.Popen([cli, "--path", path, "--scene-only", "--json"], stdout=subprocess.PIPE)
    stdout, _ = process.communicate()
    if process.returncode:
        raise RuntimeError("Post-clean scan failed")
    report = json.loads(stdout.decode("utf-8"))
    if report["files_scanned"] != 1 or report["threats_found"] != 0:
        raise RuntimeError("Known signatures remain or output scan was skipped; cleanup not accepted")


def main(job_path):
    with open(job_path, "r") as stream:
        job = json.load(stream)
    profile = load_profile(job["profile"], job["plugin"], job["maya_version"])
    if not job["files"] or any(not source.lower().endswith((".ma", ".mb")) for source in job["files"]):
        raise RuntimeError("Native cleanup accepts only .ma/.mb scene files")
    # Do not let unrelated convenience hooks delete unknown/Turtle nodes, alter
    # model panels or remove local user scripts as a side effect of this operation.
    os.environ["MAYA_UMBRELLA_DISABLE_ALL_HOOKS"] = "1"
    import maya.standalone
    from maya import cmds
    maya.standalone.initialize()
    try:
        if str(cmds.about(version=True)) != profile["maya_version"] or int(cmds.about(apiVersion=True)) != profile["api_version"]:
            raise RuntimeError("Maya version/API mismatch; no scenes were opened")
        for entry in profile["plugins"]:
            cmds.loadPlugin(entry["path"], quiet=True)
        verify_maya(cmds, profile)
        # The offline pre-open guard correctly rejects infected .mb. Disable it
        # only inside this dedicated process; script-node execution stays off.
        cmds.umbrellaDisable()
        outputs = []
        for index, source in enumerate(job["files"]):
            verify_maya(cmds, profile)
            scene_dir = tempfile.mkdtemp(prefix="scene-%d-" % index, dir=job["output_dir"])
            working = os.path.join(scene_dir, "input" + os.path.splitext(source)[1])
            output = os.path.join(scene_dir, "cleaned" + os.path.splitext(source)[1])
            original_hash = digest(source)
            shutil.copyfile(source, working)
            if digest(working) != original_hash or digest(source) != original_hash:
                raise RuntimeError("Source changed during snapshot; no source file was modified")
            cmds.file(new=True, force=True)
            cmds.file(working, open=True, force=True, executeScriptNodes=False,
                      prompt=False, loadReferenceDepth="none", ignoreVersion=False)
            verify_scene(cmds)
            verify_maya(cmds, profile)
            cmds.umbrellaFixScene()
            verify_scene(cmds)
            verify_maya(cmds, profile)
            cmds.file(rename=output)
            cmds.file(save=True, force=True, type="mayaBinary" if source.lower().endswith(".mb") else "mayaAscii")
            verify_scene(cmds)
            verify_maya(cmds, profile)
            cmds.file(new=True, force=True)
            cmds.file(output, open=True, force=True, executeScriptNodes=False,
                      prompt=False, loadReferenceDepth="none", ignoreVersion=False)
            verify_scene(cmds)
            verify_maya(cmds, profile)
            scan_output(job["cli"], output)
            if digest(source) != original_hash:
                raise RuntimeError("Source changed externally during processing; output not accepted")
            outputs.append({"source": source, "source_sha256": original_hash,
                            "output": output, "output_sha256": digest(output)})
            cmds.file(new=True, force=True)
        # The marker is written only after every scene passes. Without it, all
        # generated files are unaccepted intermediates, never replacement assets.
        with open(os.path.join(job["output_dir"], "verified-results.json"), "w") as stream:
            json.dump({"environment": profile, "results": outputs}, stream, indent=2)
        print("Native copy-only cleanup passed; review outputs in " + job["output_dir"])
    finally:
        maya.standalone.uninitialize()


if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise SystemExit("Expected one job manifest from umbrella-maya")
    main(sys.argv[1])
