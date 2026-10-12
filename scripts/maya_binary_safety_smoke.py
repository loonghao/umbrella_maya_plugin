"""Real-Maya callback regression on a disposable synthetic .mb, never user assets.

This validates the Maya 2024 CI host only, not any production environment.
"""
import hashlib
from pathlib import Path
import tempfile

import maya_standalone_smoke as host


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    args = host.parse_args()
    package = host.resolve_package(Path(args.package_root), args.maya_version, args.platform)
    plugin = host.resolve_plugin(package, args.platform).resolve()
    host.configure_environment(package, plugin, args.platform)
    import maya.standalone
    from maya import cmds
    maya.standalone.initialize()
    try:
        host.assert_maya_version(args.maya_version)
        host.load_plugin(plugin)
        cmds.umbrellaDisable()
        with tempfile.TemporaryDirectory(prefix="umbrella-mb-guard-") as directory:
            scene = Path(directory) / "synthetic.mb"
            cmds.file(new=True, force=True)
            # scriptType=0 is demand-only; no viral code is executed by this test.
            cmds.scriptNode(name="safetyFixture", scriptType=0, sourceType="python",
                            beforeScript="import vaccine")
            cmds.createNode("transform", name="keep_me")
            cmds.file(rename=str(scene))
            cmds.file(save=True, type="mayaBinary", force=True)
            original = digest(scene)
            cmds.file(new=True, force=True)
            cmds.umbrellaEnable()
            try:
                cmds.file(str(scene), open=True, force=True, executeScriptNodes=False, prompt=False)
            except RuntimeError:
                pass
            assert digest(scene) == original, "BeforeOpenCheck modified the binary scene"
            assert not cmds.objExists("keep_me"), "Blocked scene was loaded"
            assert cmds.file(query=True, sceneName=True) != str(scene), "Blocked scene became current"
            cmds.umbrellaDisable()
            cmds.file(new=True, force=True)
        print("[ok] Real Maya blocked synthetic infected .mb and preserved its bytes")
    finally:
        maya.standalone.uninitialize()


if __name__ == "__main__":
    main()
