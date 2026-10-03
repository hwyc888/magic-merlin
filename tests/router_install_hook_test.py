#!/usr/bin/env python3
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

REPO = Path(__file__).resolve().parents[1]
SHELL = os.environ.get("ROUTER_TEST_SHELL") or shutil.which("sh")
if not SHELL and os.name == "nt":
    SHELL = r"C:\\Program Files\\Git\\bin\\bash.exe"


def shell_path(path):
    value = str(path).replace("\\", "/")
    if len(value) > 1 and value[1] == ":":
        value = "/" + value[0].lower() + value[2:]
    return value


class KoolCenterHookTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="magic-install-hook-")
        self.root = Path(self.temp.name)
        (self.root / "jffs/scripts").mkdir(parents=True)
        (self.root / "koolshare/bin").mkdir(parents=True)
        (self.root / "nvram").mkdir()
        (self.root / "nvram/jffs2_scripts").write_text("0", encoding="utf8")
        for name in ("ks-services-start.sh", "ks-wan-start.sh", "ks-nat-start.sh"):
            helper = self.root / "koolshare/bin" / name
            helper.write_text("#!/bin/sh\n", encoding="utf8")
            helper.chmod(0o644)

        source = (REPO / "router-plugin/install.sh").read_text(encoding="utf8")
        start = source.index("ensure_jffs_hook() {")
        end = source.index("\ninstall_now() {", start)
        functions = source[start:end]
        functions = functions.replace("/jffs/scripts", "$TEST_ROOT/jffs/scripts")
        functions = functions.replace("/koolshare/bin", "$TEST_ROOT/koolshare/bin")
        root = shell_path(self.root)
        harness = f"""#!/bin/sh
TEST_ROOT='{root}'
nvram() {{
    case "$1" in
        get) cat "$TEST_ROOT/nvram/$2" 2>/dev/null ;;
        set)
            pair="$2"
            printf '%s' "${{pair#*=}}" > "$TEST_ROOT/nvram/${{pair%%=*}}"
            ;;
        commit)
            count="$(cat "$TEST_ROOT/nvram/commits" 2>/dev/null)"
            count=${{count:-0}}
            printf '%s' "$((count + 1))" > "$TEST_ROOT/nvram/commits"
            ;;
    esac
}}
echo_date() {{ :; }}
sync() {{ :; }}
{functions}
ensure_koolcenter_boot_hooks
"""
        self.script = self.root / "run.sh"
        self.script.write_text(harness, encoding="utf8")

    def tearDown(self):
        self.temp.cleanup()

    def run_hook(self):
        result = subprocess.run(
            [SHELL, shell_path(self.script)],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            timeout=5,
        )
        self.assertEqual(result.returncode, 0, result.stderr)

    def command(self, name):
        return {
            "services-start": f"{shell_path(self.root)}/koolshare/bin/ks-services-start.sh",
            "wan-start": f"{shell_path(self.root)}/koolshare/bin/ks-wan-start.sh start",
            "nat-start": f"{shell_path(self.root)}/koolshare/bin/ks-nat-start.sh start_nat",
        }[name]

    def test_creates_official_koolcenter_boot_chain_and_enables_jffs(self):
        self.run_hook()
        self.assertEqual((self.root / "nvram/jffs2_scripts").read_text(), "1")
        self.assertEqual((self.root / "nvram/commits").read_text(), "1")
        for name in ("services-start", "wan-start", "nat-start"):
            hook = self.root / "jffs/scripts" / name
            hook_text = hook.read_text(encoding="utf8")
            self.assertTrue(hook_text.startswith("#!/bin/sh\n"))
            self.assertEqual(hook_text.count(self.command(name)), 1)
            self.assertTrue(os.access(hook, os.X_OK))

    def test_preserves_user_hook_content_and_is_idempotent(self):
        service = self.root / "jffs/scripts/services-start"
        service.write_text("#!/bin/sh\necho user-service\n", encoding="utf8")
        nat = self.root / "jffs/scripts/nat-start"
        nat.write_text("echo user-nat\n", encoding="utf8")
        self.run_hook()
        self.run_hook()
        service_text = service.read_text(encoding="utf8")
        nat_text = nat.read_text(encoding="utf8")
        self.assertIn("echo user-service", service_text)
        self.assertIn("echo user-nat", nat_text)
        self.assertEqual(service_text.count(self.command("services-start")), 1)
        self.assertEqual(nat_text.count(self.command("nat-start")), 1)
        self.assertEqual((self.root / "nvram/commits").read_text(), "1")


if __name__ == "__main__":
    unittest.main(verbosity=2)
