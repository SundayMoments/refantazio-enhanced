# Source publication review

The public project is **ReFantazio Enhanced**, an independent Luma/MetaphorFix
fork. Runtime offline behavior and public source hosting are separate concerns.

The publication review covers tracked source, documentation, vendored notices,
patches, and the complete reachable history of the branch being published.
Commit attribution uses the maintainer's public GitHub handle and GitHub's
no-reply email address. A public repository necessarily associates the project
with its GitHub owner; this is not an anonymous publication.

The initial public branch starts with a clean source snapshot. Earlier local
development branches and tags are retained locally and are not pushed. Only
the explicitly reviewed public branch is included in the initial push.

Excluded from source control and publication:

- Game executables, assets, saves and personal settings.
- Build outputs, downloaded runtimes, archives, PDBs and memory dumps.
- Local logs, installation manifests, original-file backups and screenshots.
- Local upstream/reference clones, editor state and agent configuration.
- Environment files, authentication tokens and credentials.

The review checks for personal account paths, Steam account IDs, common token
formats and private-key material. Copyright notices, upstream author names and
upstream source links are deliberately preserved. Generic installation paths
and documented test hardware/software versions are not personal account data.

Only source is published. No release binary or local build archive is uploaded.
Compiled files can embed absolute paths even when their sources do not; binary
releases require a separate artifact review and appropriate third-party notices.

This review is evidence about this publication, not a guarantee that future
commits or attachments cannot disclose information. Review logs, screenshots,
issue attachments and binary artifacts before publishing them.
