import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location('runtime', Path(__file__).resolve().parents[1] / 'tools/runtime.py')
runtime = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runtime)


class RuntimeTests(unittest.TestCase):
    def test_requirement_is_read_from_the_binary(self):
        self.assertEqual(runtime.required_version(b'prefix RE_MSVC_RUNTIME_MIN=14.44.35220.0\0 suffix'), (14, 44, 35220, 0))

    def test_missing_or_conflicting_requirements_fail_closed(self):
        for data in (b'old addon', b'RE_MSVC_RUNTIME_MIN=14.44.35220.0\0 RE_MSVC_RUNTIME_MIN=14.51.1.0'):
            with self.assertRaises(RuntimeError):
                runtime.required_version(data)

    def test_servicing_and_abi_boundaries(self):
        minimum = (14, 44, 35220, 0)
        for old in ((0, 0, 0, 0), (14, 39, 99999, 0), (14, 44, 35211, 0), (15, 0, 0, 0)):
            self.assertFalse(runtime.compatible(old, minimum))
        for valid in (minimum, (14, 44, 35220, 1), (14, 51, 36247, 0)):
            self.assertTrue(runtime.compatible(valid, minimum))


if __name__ == '__main__':
    unittest.main()
