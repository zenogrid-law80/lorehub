import fcntl
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import unittest
from unittest import mock

HELPER = Path(__file__).resolve().parents[1] / 'scripts/repository-folder-archive.py'
spec = importlib.util.spec_from_file_location('folder_archive', HELPER)
archive = importlib.util.module_from_spec(spec)
spec.loader.exec_module(archive)


class FolderArchives(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.source = self.root / 'source'
        (self.source / '.lore').mkdir(parents=True)
        (self.source / '.lore/id').write_bytes(bytes(range(16)))
        (self.source / '.lore/lock').touch()
        (self.source / '.lore/config.toml').write_text('remote_url = "lores://example.test"\n')
        (self.source / '미커밋 파일.txt').write_text('uncommitted\n')
        (self.source / 'empty').touch()
        (self.source / 'empty-dir').mkdir()
        (self.source / 'script.sh').write_text('#!/bin/sh\necho test\n')
        (self.source / 'script.sh').chmod(0o755)
        (self.source / '.hidden').write_bytes(os.urandom(2 * 1024 * 1024))
        self.output = self.root / 'repository.tar.gz'
        self.request = {'source': str(self.source), 'archive': str(self.output),
                        'resource_id': 'urc-' + bytes(range(16)).hex()}

    def create(self):
        return archive.create_archive(self.request)

    def verify_request(self, result):
        return {'archive': str(self.output), 'archive_sha256': result['archive_sha256']}

    def test_round_trip_preserves_hidden_files_uncommitted_content_modes_and_identity(self):
        result = self.create()
        verified = archive.verify_archive(self.verify_request(result))
        self.assertEqual(verified['file_count'], 7)
        target = self.root / 'restored'
        restored = archive.restore_archive(dict(self.verify_request(result), target=str(target)))
        self.assertEqual(restored['repository_id'], self.request['resource_id'])
        for path in self.source.rglob('*'):
            copy = target / path.relative_to(self.source)
            if path.is_file():
                self.assertEqual(copy.read_bytes(), path.read_bytes())
                self.assertEqual(copy.stat().st_mode & 0o777, path.stat().st_mode & 0o777)
            else:
                self.assertTrue(copy.is_dir())
        self.assertFalse((target / '.lorehub-restore-incomplete').exists())

    def test_real_cli_json_protocol(self):
        completed = subprocess.run(['python3', '-I', str(HELPER)], input=json.dumps(dict(self.request, action='create')),
                                   text=True, capture_output=True)
        self.assertEqual(completed.returncode, 0, completed.stderr)
        result = json.loads(completed.stdout)
        self.assertEqual(result['kind'], 'folder-tar-gzip')
        self.assertNotIn('entries', result)

    def test_existing_archive_and_target_are_never_overwritten(self):
        result = self.create()
        original = self.output.read_bytes()
        with self.assertRaises(FileExistsError):
            self.create()
        self.assertEqual(self.output.read_bytes(), original)
        target = self.root / 'restored'
        target.mkdir()
        (target / 'keep').write_text('keep')
        with self.assertRaisesRegex(ValueError, 'already exists'):
            archive.restore_archive(dict(self.verify_request(result), target=str(target)))
        self.assertEqual((target / 'keep').read_text(), 'keep')

    def test_changed_archive_is_rejected_before_files_are_extracted(self):
        result = self.create()
        with self.output.open('ab') as stream:
            stream.write(b'corruption')
        target = self.root / 'failed'
        with self.assertRaisesRegex(ValueError, 'checksum'):
            archive.restore_archive(dict(self.verify_request(result), target=str(target)))
        self.assertTrue((target / '.lorehub-restore-incomplete').exists())
        self.assertFalse((target / '.lore').exists())

    def test_source_identity_mismatch(self):
        with self.assertRaisesRegex(ValueError, 'different Lore repository'):
            archive.create_archive(dict(self.request, resource_id='urc-wrong'))
        self.assertFalse(self.output.exists())

    def test_busy_repository_fails_without_publishing_archive(self):
        with (self.source / '.lore/lock').open('rb') as lock:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            with self.assertRaisesRegex(ValueError, 'in use'):
                self.create()
        self.assertFalse(self.output.exists())

    def test_symlinks_are_rejected_without_following_external_files(self):
        (self.source / 'external').symlink_to(self.root)
        with self.assertRaisesRegex(ValueError, 'Symbolic links'):
            self.create()
        self.assertFalse(self.output.exists())

    def test_changes_during_backup_are_rejected(self):
        real_digest = archive.digest_file
        def changed(path):
            if Path(path).name == 'empty':
                Path(path).write_text('edited while backing up')
            return real_digest(path)
        with mock.patch.object(archive, 'digest_file', side_effect=changed):
            with self.assertRaisesRegex(ValueError, 'changed during backup'):
                self.create()
        self.assertFalse(self.output.exists())

    def test_nested_backup_folder_is_rejected(self):
        with self.assertRaisesRegex(ValueError, 'outside the source'):
            archive.create_archive(dict(self.request, archive=str(self.source / 'nested.tar.gz')))

    def test_unsafe_tar_members_are_rejected(self):
        for name, kind in [('repository/../../escape', tarfile.REGTYPE),
                           ('repository/link', tarfile.SYMTYPE),
                           ('repository/hard', tarfile.LNKTYPE),
                           ('/absolute', tarfile.REGTYPE),
                           ('repository/fifo', tarfile.FIFOTYPE)]:
            with self.subTest(name=name):
                stream = io.BytesIO()
                with tarfile.open(fileobj=stream, mode='w:gz') as output:
                    header = tarfile.TarInfo(name)
                    header.type = kind
                    header.linkname = '/outside'
                    output.addfile(header, io.BytesIO())
                stream.seek(0)
                with self.assertRaises(ValueError):
                    archive.read_archive(stream)
        self.assertFalse((self.root / 'escape').exists())

    def test_per_file_checksums_reject_tampering_even_with_matching_outer_digest(self):
        self.create()
        modified = self.root / 'modified.tar.gz'
        with tarfile.open(self.output, 'r:gz') as source, tarfile.open(modified, 'w:gz') as output:
            for member in source:
                data = source.extractfile(member).read() if member.isfile() else None
                if member.name == 'repository/empty':
                    data = b'changed'
                    member.size = len(data)
                output.addfile(member, io.BytesIO(data) if data is not None else None)
        with self.assertRaisesRegex(ValueError, 'checksums'):
            archive.verify_archive({'archive': str(modified), 'archive_sha256': hashlib.sha256(modified.read_bytes()).hexdigest()})


if __name__ == '__main__':
    unittest.main()
