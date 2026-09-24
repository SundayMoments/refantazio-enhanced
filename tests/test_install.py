"""Installer regression tests in temporary folders; never touch the game directory."""
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('installer', Path(__file__).resolve().parents[1]/'tools/install.py')
installer = importlib.util.module_from_spec(spec); spec.loader.exec_module(installer)

class InstallerTests(unittest.TestCase):
    def setUp(self):
        self.base = Path(__file__).resolve().parents[1]/'artifacts/tests'
        self.base.mkdir(parents=True,exist_ok=True)
        self.temp = tempfile.TemporaryDirectory(dir=self.base)
        self.root = Path(self.temp.name)
        self.source = self.root/'source'; self.source.write_bytes(b'new build')
        self.target = self.root/'target'; self.target.write_bytes(b'known good')
    def tearDown(self):
        assert self.root.resolve().is_relative_to(self.base.resolve()) and self.root.resolve() != self.base.resolve()
        self.temp.cleanup()
    def test_successful_replace(self):
        installer.atomic_copy(self.source,self.target)
        self.assertEqual(self.target.read_bytes(),b'new build')
        self.assertFalse(self.target.with_name('target.refantazio-stage').exists())
    def test_copy_failure_preserves_working_binary(self):
        def fail(input,output):
            output.write(b'partial'); raise OSError('simulated disk failure')
        with patch.object(installer.shutil,'copyfileobj',side_effect=fail):
            with self.assertRaises(OSError): installer.atomic_copy(self.source,self.target)
        self.assertEqual(self.target.read_bytes(),b'known good')
        self.assertFalse(self.target.with_name('target.refantazio-stage').exists())
    def test_existing_staging_file_preserved(self):
        staged=self.target.with_name('target.refantazio-stage'); staged.write_bytes(b'previous transaction')
        with self.assertRaises(RuntimeError): installer.atomic_copy(self.source,self.target)
        self.assertEqual(staged.read_bytes(),b'previous transaction')
    def test_interrupted_transaction_accepts_only_known_states(self):
        entry={'installed':installer.digest(self.source), 'installed_before':installer.digest(self.target)}
        self.assertTrue(installer.matches_entry(self.target,entry))
        installer.atomic_copy(self.source,self.target)
        self.assertTrue(installer.matches_entry(self.target,entry))
        self.target.write_bytes(b'user modification')
        self.assertFalse(installer.matches_entry(self.target,entry))
    def test_payload_cannot_escape_game_directory(self):
        with patch.object(installer,'GAME',self.root):
            for path in ('../outside.dll',r'C:\outside.dll','unrelated.exe','Luma/../../outside.dll'):
                with self.assertRaises(RuntimeError): installer.game_file(path)

if __name__ == '__main__': unittest.main()
