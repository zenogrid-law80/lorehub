#!/usr/bin/env python3
"""LoreHub folder archives. JSON request on stdin; JSON result on stdout.

Uses only Python 3.9+ standard libraries. Never follows links or extracts tar paths
with extractall. A local folder snapshot preserves local files, not remote history.
"""
import contextlib
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import stat
import sys
import tarfile
import tempfile

FORMAT = 2
MAX_MANIFEST = 32 * 1024 * 1024
MAX_ENTRIES = 250000
CHUNK = 1024 * 1024


def require(condition, message):
    if not condition:
        raise ValueError(message)


def real_directory(path):
    path = Path(path)
    require(path.is_absolute(), "Directory must be an absolute path")
    require(not path.is_symlink() and path.is_dir(), "Directory is missing or is a symbolic link")
    require(path == path.resolve(), "Directory must not contain symbolic links")
    return path


def fingerprint(info):
    return (info.st_dev, info.st_ino, info.st_mode, info.st_size,
            info.st_mtime_ns, info.st_ctime_ns)


def open_regular(path):
    fd = os.open(path, os.O_RDONLY | getattr(os, 'O_NOFOLLOW', 0) | getattr(os, 'O_NONBLOCK', 0))
    stream = os.fdopen(fd, 'rb')
    if not stat.S_ISREG(os.fstat(fd).st_mode):
        stream.close()
        raise ValueError("Only regular files can be archived")
    return stream


def digest_stream(stream, output=None):
    digest = hashlib.sha256()
    size = 0
    while True:
        block = stream.read(CHUNK)
        if not block:
            return digest.hexdigest(), size
        digest.update(block)
        size += len(block)
        if output is not None:
            output.write(block)


def digest_file(path):
    with open_regular(path) as stream:
        return digest_stream(stream)[0]


def inventory(source):
    entries = {}
    def visit(directory):
        with os.scandir(directory) as scan:
            children = sorted(scan, key=lambda entry: entry.name)
        for child in children:
            path = Path(child.path)
            relative = path.relative_to(source).as_posix()
            info = child.stat(follow_symlinks=False)
            require(stat.S_ISDIR(info.st_mode) or stat.S_ISREG(info.st_mode),
                    "Symbolic links and special files are not supported: " + relative)
            require(len(entries) < MAX_ENTRIES, "Too many files for a folder archive")
            entries[relative] = fingerprint(info)
            if stat.S_ISDIR(info.st_mode):
                visit(path)
    visit(source)
    return entries


def source_identity(source):
    real_directory(source / '.lore')
    with open_regular(source / '.lore' / 'id') as stream:
        identity = stream.read(17)
    require(len(identity) == 16, "Invalid .lore/id; select a Lore repository folder")
    return 'urc-' + identity.hex()


@contextlib.contextmanager
def repository_lock(source):
    import fcntl
    with open_regular(source / '.lore' / 'lock') as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            raise ValueError("The repository is in use. Close Lore operations and retry.") from None
        try:
            yield
        finally:
            fcntl.flock(lock, fcntl.LOCK_UN)


def inspect_source(request):
    source = real_directory(request['source'])
    identity = source_identity(source)
    require(identity == request['resource_id'], "Local folder belongs to a different Lore repository")
    return {'source_path': request.get('source_label', str(source)), 'repository_id': identity}


class HashReader:
    def __init__(self, stream):
        self.stream = stream
        self.digest = hashlib.sha256()
    def read(self, count=-1):
        data = self.stream.read(count)
        self.digest.update(data)
        return data


def sync_directory(directory):
    fd = os.open(directory, os.O_RDONLY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def create_archive(request):
    source = real_directory(request['source'])
    archive = Path(request['archive'])
    parent = real_directory(archive.parent)
    require(source != parent and source not in parent.parents, "Backup storage must be outside the source folder")
    server = request.get('kind') == 'server-store-tar-gzip'
    if server:
        validate_server_tree(source)
    else:
        inspect_source(request)
    prefix = 'server-data/' if server else 'repository/'
    with contextlib.nullcontext() if server else repository_lock(source):
        before = inventory(source)
        records = []
        with tempfile.NamedTemporaryFile(prefix='.archive-', dir=parent, delete=False) as temporary:
            temporary_path = Path(temporary.name)
        try:
            with tarfile.open(temporary_path, 'w:gz', dereference=False, format=tarfile.PAX_FORMAT) as output:
                for name, expected in before.items():
                    path = source / name
                    info = path.lstat()
                    require(fingerprint(info) == expected, "Source changed during backup; retry")
                    header = tarfile.TarInfo(prefix + name)
                    header.mode = stat.S_IMODE(info.st_mode) & 0o777
                    header.mtime = info.st_mtime
                    record = {'path': name, 'mode': header.mode}
                    if stat.S_ISDIR(info.st_mode):
                        header.type = tarfile.DIRTYPE
                        record['kind'] = 'directory'
                        output.addfile(header)
                    else:
                        header.size = info.st_size
                        with open_regular(path) as stream:
                            require(fingerprint(os.fstat(stream.fileno())) == expected, "Source changed during backup; retry")
                            reader = HashReader(stream)
                            output.addfile(header, reader)
                            require(fingerprint(os.fstat(stream.fileno())) == expected, "Source changed during backup; retry")
                        record.update(kind='file', size=header.size, sha256=reader.digest.hexdigest())
                    records.append(record)
                manifest = {'format': 3 if server else FORMAT, 'kind': 'server-store-tar-gzip' if server else 'folder-tar-gzip',
                            'repository_id': request['resource_id'], 'source_path': request.get('source_label', str(source)),
                            'file_count': sum(row['kind'] == 'file' for row in records),
                            'total_bytes': sum(row.get('size', 0) for row in records), 'entries': records}
                if server:
                    manifest['server'] = request['server']
                    manifest['scope'] = 'all-local-server-repositories'
                encoded = json.dumps(manifest, ensure_ascii=True, separators=(',', ':')).encode()
                require(len(encoded) <= MAX_MANIFEST, "Folder manifest is too large")
                header = tarfile.TarInfo('manifest.json')
                header.size = len(encoded)
                header.mode = 0o600
                output.addfile(header, io.BytesIO(encoded))
            require(inventory(source) == before, "Source changed during backup; retry")
            # Working files can be edited without taking Lore's lock. Detect those changes too.
            for row in records:
                if row['kind'] == 'file':
                    require(digest_file(source / row['path']) == row['sha256'], "Source changed during backup; retry")
            require(inventory(source) == before, "Source changed during backup; retry")
            with open(temporary_path, 'rb') as stream:
                os.fsync(stream.fileno())
            os.link(temporary_path, archive)  # Atomic publication; never overwrite an archive.
            sync_directory(parent)
            result = summary(manifest)
            result.update(archive_sha256=digest_file(archive), size_bytes=archive.stat().st_size)
            return result
        finally:
            temporary_path.unlink(missing_ok=True)


def safe_relative(name):
    path = PurePosixPath(name)
    require(name and not path.is_absolute() and all(part not in ('', '.', '..') for part in name.split('/'))
            and '\\' not in name and '\x00' not in name, "Unsafe archive path")
    return path


def summary(manifest):
    return {key: value for key, value in manifest.items() if key != 'entries'}


def read_archive(stream, destination=None):
    records = []
    seen = set()
    manifest = None
    repository_id = None
    directory_modes = []
    prefix = None
    with tarfile.open(fileobj=stream, mode='r|gz') as archive:
        for member in archive:
            require(manifest is None, "Unexpected archive entry after manifest")
            require(len(records) < MAX_ENTRIES + 1, "Too many archive entries")
            require(member.name not in seen, "Duplicate archive path")
            seen.add(member.name)
            if member.name == 'manifest.json':
                require(member.isfile() and 0 < member.size <= MAX_MANIFEST, "Invalid archive manifest")
                manifest = json.load(archive.extractfile(member))
                continue
            if prefix is None:
                prefix = 'server-data/' if member.name.startswith('server-data/') else 'repository/'
            require(member.name.startswith(prefix), "Unexpected archive entry")
            name = member.name[len(prefix):].rstrip('/') if member.isdir() else member.name[len(prefix):]
            relative = safe_relative(name)
            require(member.isfile() or member.isdir(), "Archive links and special files are not supported")
            record = {'path': name, 'mode': member.mode & 0o777, 'kind': 'directory' if member.isdir() else 'file'}
            target = destination.joinpath(*relative.parts) if destination else None
            if target:
                target.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
            if member.isdir():
                if target:
                    target.mkdir(mode=0o700, exist_ok=True)
                    directory_modes.append((target, record['mode']))
            else:
                with archive.extractfile(member) as content:
                    if name == '.lore/id':
                        require(member.size == 16, "Invalid repository ID in archive")
                        data = content.read()
                        repository_id = 'urc-' + data.hex()
                        content = io.BytesIO(data)
                    if target:
                        with target.open('xb') as output:
                            digest, size = digest_stream(content, output)
                            output.flush()
                            os.fsync(output.fileno())
                        os.chmod(target, record['mode'])
                        os.utime(target, (member.mtime, member.mtime))
                    else:
                        digest, size = digest_stream(content)
                require(size == member.size, "Truncated archive file")
                record.update(size=size, sha256=digest)
            records.append(record)
    server = prefix == 'server-data/'
    require(manifest is not None and manifest.get('format') == (3 if server else FORMAT)
            and manifest.get('kind') == ('server-store-tar-gzip' if server else 'folder-tar-gzip'), "Unsupported folder archive")
    require(manifest.get('entries') == records, "Archive file checksums or metadata do not match")
    if server:
        require(manifest.get('repository_id') == 'server:local' and manifest.get('scope') == 'all-local-server-repositories', "Invalid server archive scope")
        types = {row['path']: row['kind'] for row in records}
        require(types.get('immutable/immutable/index') == 'directory'
                and types.get('mutable/mutable/index') == 'directory'
                and types.get('mutable/mutable/version') == 'file', "Server stores are incomplete")
    else:
        require(repository_id and repository_id == manifest.get('repository_id'), "Archive repository identity mismatch")
    require(manifest.get('file_count') == sum(row['kind'] == 'file' for row in records)
            and manifest.get('total_bytes') == sum(row.get('size', 0) for row in records), "Archive size/count mismatch")
    # Modes are applied last so read-only directories do not prevent extraction.
    for path, mode in reversed(directory_modes):
        os.chmod(path, mode)
    return summary(manifest)


def verify_archive(request, destination=None):
    with open_regular(request['archive']) as stream:
        before = fingerprint(os.fstat(stream.fileno()))
        digest, _ = digest_stream(stream)
        require(digest == request['archive_sha256'], "Backup archive checksum mismatch")
        stream.seek(0)
        result = read_archive(stream, destination)
        require(fingerprint(os.fstat(stream.fileno())) == before, "Archive changed during verification")
        return result


def restore_archive(request):
    target = Path(request['target'])
    parent = real_directory(target.parent)
    require(not os.path.lexists(target), "Restore folder already exists; choose a new name")
    # Reserve the target atomically. Incomplete output is private and has a marker.
    target.mkdir(mode=0o700)
    marker = target / '.lorehub-restore-incomplete'
    marker.write_text('Restore incomplete. Do not use this folder.\n')
    staging = Path(tempfile.mkdtemp(prefix='.restoring-', dir=target))
    try:
        result = verify_archive(request, staging)
        # Do not overwrite even names introduced in the target while we were extracting.
        require(set(target.iterdir()) == {marker, staging}, "Restore target changed during extraction")
        for child in staging.iterdir():
            require(child.name != marker.name and not os.path.lexists(target / child.name), "Restore target name collision")
        for child in list(staging.iterdir()):
            os.rename(child, target / child.name)
        staging.rmdir()
        marker.unlink()
        sync_directory(target)
        sync_directory(parent)
        result['target_path'] = str(target)
        return result
    except Exception:
        # Keep the marker and partial output for inspection; never expose a successful restore.
        raise


def docker(*args, timeout=60):
    import subprocess
    binary = os.environ.get('LOREHUB_BACKUP_DOCKER', 'docker')
    result = subprocess.run([binary, *args], stdin=subprocess.DEVNULL, capture_output=True, timeout=timeout)
    require(result.returncode == 0, 'Docker operation failed: ' + result.stderr.decode(errors='replace')[:600])
    return result.stdout


def container_info(container):
    import re
    require(re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9_.-]{0,127}', container), 'Invalid configured server container')
    return json.loads(docker('inspect', container))[0]


def validate_server_tree(source):
    require((source / 'immutable/immutable/index').is_dir()
            and (source / 'mutable/mutable/index').is_dir()
            and (source / 'mutable/mutable/version').is_file(), 'Server immutable/mutable stores are missing')
    require(set(p.name for p in source.iterdir()) == {'immutable', 'mutable'}, 'Unexpected server store layout')


def inspect_server(request):
    import tomllib
    info = container_info(request['container'])
    require(info['Config'].get('Labels', {}).get('com.docker.compose.service') == 'lore-server-local',
            'Only the configured local Lore server can be snapshotted')
    require(info['Config'].get('Entrypoint') == ['loreserver'] and not info['Config'].get('Cmd'),
            'Custom server command requires a separate backup configuration')
    env = dict(value.split('=', 1) for value in info['Config'].get('Env', []) if '=' in value)
    require(env.get('LORE_CONFIG_PATH') == '/etc/lore/config' and env.get('LORE_ENV') == 'docker', 'Unexpected Lore config location')
    require(not any(key.upper().startswith(('LORE__IMMUTABLE_STORE', 'LORE__MUTABLE_STORE', 'LORE__PLUGINS'))
                    for key in env), 'Store environment overrides are not supported by this snapshot adapter')
    # Read only the effective local override, not credentials/certificates or the whole container.
    config_tar = docker('cp', info['Id'] + ':/etc/lore/config/local.toml', '-')
    with tarfile.open(fileobj=io.BytesIO(config_tar), mode='r:') as bundle:
        members = bundle.getmembers()
        require(len(members) == 1 and members[0].isfile() and members[0].size < MAX_MANIFEST, 'Unexpected server config file')
        config = tomllib.loads(bundle.extractfile(members[0]).read().decode())
    for kind in ('immutable', 'mutable'):
        store = config.get(kind + '_store', {})
        require(store.get('mode') == 'local' and store.get('local', {}).get('path') == '/var/lib/lore/' + kind,
                'Server must use the expected local immutable and mutable store paths')
    require(not config.get('plugins'), 'Plugin store overrides require a separate backup adapter')
    mounts = [mount for mount in info['Mounts'] if mount['Destination'] == '/var/lib/lore']
    require(len(mounts) == 1 and mounts[0]['Type'] == 'volume', 'Expected a dedicated Docker data volume')
    require(not any(mount['Destination'].startswith('/var/lib/lore/') for mount in info['Mounts']), 'Nested data mounts are not supported')
    volume = mounts[0]['Name']
    # Another process using this volume would invalidate an offline snapshot.
    running = docker('ps', '-q', '--filter', 'volume=' + volume).decode().split()
    for identifier in running:
        require(info['Id'].startswith(identifier), 'Another running container uses the server volume')
    require(not info['State'].get('Paused') and not info['State'].get('Restarting'), 'Server is paused or restarting')
    return {'container_id': info['Id'], 'container_name': info['Name'].lstrip('/'),
            'image_id': info['Image'], 'volume': volume, 'data_path': '/var/lib/lore',
            'running': info['State']['Running'], 'scope': 'all-local-server-repositories'}


@contextlib.contextmanager
def server_snapshot_lock(root):
    import fcntl
    with (root / '.server-snapshot.lock').open('a+b') as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            raise ValueError('A server snapshot is already running') from None
        yield


def restart_record(root):
    return root / '.server-snapshot-recovery.json'


def recover_server(request):
    root = real_directory(request['root'])
    with server_snapshot_lock(root):
        record = restart_record(root)
        if not record.exists():
            return {'recovered': False}
        with open_regular(record) as stream:
            pending = json.load(stream)
        info = container_info(pending['container_id'])
        require(info['Config'].get('Labels', {}).get('com.docker.compose.service') == 'lore-server-local', 'Recovery container identity mismatch')
        if not info['State']['Running']:
            docker('start', pending['container_id'])
        require(container_info(pending['container_id'])['State']['Running'], 'Server restart failed; operator action required')
        record.unlink()
        sync_directory(root)
        return {'recovered': True}


def snapshot_server(request):
    import signal
    root = real_directory(request['root'])
    archive = Path(request['archive'])
    require(archive.parent.parent == root, 'Server archive must be under the backup root')
    metadata = inspect_server(request)
    identity = metadata['container_id']
    def interrupted(signum, frame):
        raise InterruptedError('Server snapshot interrupted')
    previous = signal.signal(signal.SIGTERM, interrupted)
    try:
        with server_snapshot_lock(root):
            require(not restart_record(root).exists(), 'An interrupted server snapshot needs recovery first')
            with tempfile.TemporaryDirectory(prefix='.server-snapshot-', dir=archive.parent) as temporary:
                source = Path(temporary) / 'data'
                source.mkdir(mode=0o700)
                record = restart_record(root)
                if metadata['running']:
                    with record.open('x') as output:
                        json.dump({'container_id': identity}, output)
                        output.flush()
                        os.fsync(output.fileno())
                    os.chmod(record, 0o600)
                    sync_directory(root)
                try:
                    if metadata['running']:
                        docker('stop', '--time', '60', identity, timeout=75)
                    stopped = container_info(identity)
                    require(not stopped['State']['Running'] and stopped['State']['ExitCode'] == 0
                            and not stopped['State'].get('OOMKilled'), 'Server did not stop cleanly; no backup was published')
                    docker('cp', identity + ':/var/lib/lore/.', str(source), timeout=1200)
                    after = container_info(identity)
                    require(not after['State']['Running'] and after['State']['StartedAt'] == stopped['State']['StartedAt'],
                            'Server restarted during the snapshot; retry')
                    require(not docker('ps', '-q', '--filter', 'volume=' + metadata['volume']).strip(), 'Another container started using the server volume during the snapshot')
                    validate_server_tree(source)
                finally:
                    if metadata['running']:
                        # Resume even after timeout, copy failure, or SIGTERM.
                        docker('start', identity)
                        require(container_info(identity)['State']['Running'], 'Server restart failed; operator action required')
                        record.unlink()
                        sync_directory(root)
                metadata.pop('running')
                return create_archive({'source': str(source), 'archive': str(archive),
                    'resource_id': 'server:local', 'kind': 'server-store-tar-gzip',
                    'source_label': metadata['container_name'] + ':/var/lib/lore', 'server': metadata})
    finally:
        signal.signal(signal.SIGTERM, previous)


def main():
    request = json.load(sys.stdin)
    action = request.get('action')
    handlers = {'inspect': inspect_source, 'create': create_archive,
                'verify': verify_archive, 'restore': restore_archive,
                'inspect-server': inspect_server, 'snapshot-server': snapshot_server, 'recover-server': recover_server}
    require(action in handlers, "Unknown folder archive action")
    print(json.dumps(handlers[action](request), ensure_ascii=True))


if __name__ == '__main__':
    try:
        main()
    except Exception as error:
        print(str(error), file=sys.stderr)
        sys.exit(1)
