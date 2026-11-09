import importlib.util
from pathlib import Path
import unittest


spec = importlib.util.spec_from_file_location(
    "backport", Path(__file__).with_name("check-sqlx-backport.py")
)
backport = importlib.util.module_from_spec(spec)
spec.loader.exec_module(backport)


class BackportReleaseTests(unittest.TestCase):
    def lock(self, sqlite):
        return {
            "package": [
                {"name": "sqlx", "version": "0.9.1"},
                {"name": "libsqlite3-sys", "version": sqlite},
            ]
        }

    def test_old_releases_still_need_backport(self):
        for version in ["0.37.0", "0.38.2"]:
            with self.subTest(version=version):
                self.assertIsNone(backport.replacement(self.lock(version)))

    def test_patched_and_future_releases_trigger_removal(self):
        for version in ["0.39.0", "0.39.1", "0.40.0", "0.100.0", "1.0.0"]:
            with self.subTest(version=version):
                self.assertEqual(
                    backport.replacement(self.lock(version)), ("0.9.1", version)
                )

    def test_missing_dependency_is_an_error_not_a_false_success(self):
        with self.assertRaises(KeyError):
            backport.replacement({"package": [{"name": "sqlx", "version": "1.0.0"}]})


if __name__ == "__main__":
    unittest.main()
