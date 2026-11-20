import importlib.util
import pathlib
import unittest

file = pathlib.Path(__file__).resolve().parents[1] / "tools" / "verify_windows_package.py"
spec = importlib.util.spec_from_file_location("package_check", file)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)

class PackageEnvironmentTests(unittest.TestCase):
    def test_uppercase_windows_environment(self):
        source = {"SYSTEMROOT": r"C:\Windows", "PATH": r"C:\BuildTools;C:\extra", "OTHER": "kept"}
        isolated = module.standalone_environment(source)
        self.assertEqual(isolated["PATH"], r"C:\Windows\System32")
        self.assertEqual(isolated["OTHER"], "kept")
        self.assertEqual(source["PATH"], r"C:\BuildTools;C:\extra")

    def test_mixed_case_and_nonstandard_system_drive(self):
        isolated = module.standalone_environment({"SystemRoot": r"D:\Windows", "Path": r"D:\toolchain"})
        self.assertEqual(isolated["PATH"], r"D:\Windows\System32")
        self.assertNotIn("Path", isolated)

    def test_windir_fallback(self):
        self.assertEqual(module.standalone_environment({"windir": r"C:\WINNT"})["PATH"], r"C:\WINNT\System32")

    def test_missing_or_relative_root_rejected(self):
        for env in [{}, {"SYSTEMROOT": "relative"}, {"SYSTEMROOT": "C:relative"}]:
            with self.assertRaises(RuntimeError):
                module.standalone_environment(env)

if __name__ == "__main__":
    unittest.main()
