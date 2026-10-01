# Lore server data backup and restore

The current backup action protects **all original data on `lore-server-local`**. This
server stores multiple repositories together in shared immutable/mutable index files.
There is no repository-per-folder boundary, so a folder archive cannot select or restore
only `game` or `developer`. All local-server repositories are included in one snapshot.

The configured source in this installation is:

| Item | Value |
| --- | --- |
| Container | `lorehub-lore-server-local-1` |
| Docker volume | `lorehub_lore_data` |
| Container data root | `/var/lib/lore` |
| Immutable store | `/var/lib/lore/immutable/immutable` |
| Mutable store | `/var/lib/lore/mutable/mutable` |
| Backup directory on the Mac | `/Users/law80/lorehub/backup` |

This is not the client checkout `/Users/law80/lorehub/game`. Files that were never sent
to the server are not included. The separate `lore-server` service using S3 and DynamoDB
is not covered; archiving its container filesystem would not protect its cloud data.

## Configuration

The coordinator requires Python 3.11+ (standard library only) and Docker access. No Lore
CLI modification or extra Lore backup command is required.

```dotenv
LOREHUB_BACKUP_DIR=/Users/law80/lorehub/backup
LOREHUB_BACKUP_SERVER_CONTAINER=lorehub-lore-server-local-1
LOREHUB_BACKUP_PYTHON=/opt/homebrew/bin/python3
LOREHUB_BACKUP_DOCKER=/usr/local/bin/docker
```

Backup storage must already exist, be owned by the service account, use an absolute path
without symbolic links, and be kept private (0700). The current adapter requires the
standard Compose `lore-server-local` service, local immutable/mutable stores at the paths
above, and a dedicated Docker volume. It rejects cloud stores, custom commands, nested
mounts, store environment overrides and other running containers sharing the volume.
Changing server storage configuration requires revisiting the adapter.

## Snapshot workflow

Use **All backups (모든 백업)** in the global navigation for backup creation,
verification and restoration. Repository navigation has no backup or restore menu;
older repository-scoped backup links open the global catalog.

Only current LoreHub administrators can create, inspect, verify or restore server
snapshots. This applies even if the user originally requested the snapshot and was later
demoted. The UI asks the administrator to acknowledge a brief outage for **all** local
server repositories. Starting a repository-scoped client-folder backup is now rejected.

1. Validate the live container configuration and volume mapping.
2. Acquire a host lock and persist a recovery record before stopping a running server.
3. Stop the server normally (60 second grace period). Lore drains connections and flushes
   immutable and mutable stores during shutdown. A forced kill, OOM or nonzero exit fails
   the snapshot; it is not published as a successful backup.
4. Copy `/var/lib/lore/.` from the stopped container to a private staging directory on the
   backup disk. Confirm the server stayed stopped and no other container started using
   the volume.
5. Restart the same container immediately after copying, including on copy failures.
6. Compress and hash the staged data, then verify the archive before recording success.

Compression runs after the server resumes. Copy duration determines downtime and depends
on data size and disk speed. The copy has a 20 minute timeout; the complete helper has a
45 minute timeout. Only one coordinator should use this catalog. The helper continues its
restart cleanup if the coordinator exits; a durable recovery record allows startup to
restart a server left stopped by an interrupted helper. If restart fails, inspect
`.server-snapshot-recovery.json` and the named container; operator intervention may be
needed. A machine/Docker outage cannot be automatically repaired by this process.

Archives use `LOREHUB_BACKUP_DIR/<backup UUID>/repository.tar.gz`, with an outer
`manifest.json`. A server archive contains `server-data/immutable`, `server-data/mutable`
and an embedded manifest. The manifest records the container image ID, volume name,
source path, whole-server scope, per-file size/mode/SHA-256 and archive SHA-256.

The outer manifest records LoreHub repository names, IDs and configuration for operator
reference. This metadata is not a transactional PostgreSQL snapshot and is not applied
by filesystem restore. Protect the LoreHub database separately with
`scripts/backup-postgres.sh`; retain both backup sets for a full deployment recovery.
Certificates, keys, auth services, container images and server configuration outside
`/var/lib/lore` must be retained independently. Restore with the recorded image version
before attempting storage format upgrades.

## Restore and validation

The UI restores into a **new offline directory**:

```text
LOREHUB_BACKUP_DIR/restored/<folder-name>/
  immutable/immutable/...
  mutable/mutable/...
```

It validates the archive checksum, every file hash, entry type, path and required store
layout. Existing destinations are refused. Failed restores retain an incomplete marker.
A successful extraction is an offline recovery copy; **the running server and its data
volume are not overwritten or switched by the UI**. Restore retains every repository ID
and all references in the archived store. Individual repositories cannot be renamed,
selected or given new identities through this physical restore.

To put an inspected restore into service, an operator should copy both store trees into
a new empty Docker volume, validate it with the recorded Lore server image in isolation,
then stop the production local server and switch its data volume. Keep the original volume
for rollback. Never copy files over an active store or combine mutable/immutable trees
from different snapshots. Review LoreHub PostgreSQL metadata and authorization before
serving an older snapshot. This release intentionally provides offline extraction and
verification; switching production volumes remains an explicit operator operation.

Tar archives reject symbolic/hard links, device nodes, FIFOs and unsafe paths. File modes
and modification times are retained. Ownership, ACLs and extended attributes are not;
set the restored volume ownership for the server account before activation. The helper
limits inventory to 250,000 entries and a 32 MiB manifest.

## Existing archives

Earlier **working-folder** archives remain readable and are labelled “not a server
backup.” They contain `.lore` and local working files and cannot restore the server data
volume. They keep their original owner/admin access rules. Native `.lorebackup` files from
the earlier CLI implementation are not supported by the folder restore path.

## API and tests

All mutations require a session and CSRF header. New snapshots use:

```text
POST /api/v1/server-backups/local
{"acknowledge_downtime":true}
```

GET `/api/v1/repository-backups` lists accessible archives. Repository-scoped lists also
show server-wide snapshots for administrators. Existing detail, verify, restore-preview,
restore and restore-status routes remain; restore input is `{"name":"server-restored"}`.
Job creation returns HTTP 202 with an ID. Lists return the newest 100 backups and 30 restores.

Run `scripts/check.sh` using Python 3.11+. Tests cover archive integrity, unsafe paths,
no-overwrite behavior, server stop/copy/start, failed copies, forced termination, restart
recovery, cloud-store rejection, admin-only access and legacy archive compatibility.
