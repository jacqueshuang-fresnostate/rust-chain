# Native Multi-platform GitHub Builds

## Sources

- GitHub-hosted runners reference: `ubuntu-24.04-arm` is an ARM64 Linux runner available to public repositories.
- Docker multi-platform GitHub Actions guide: distributing builds avoids the significant QEMU runtime cost of building multiple platforms on one runner.
- Docker GitHub Builder reference: `build.yml@v1` defaults to `distribute: true`, maps AMD64 to `ubuntu-24.04` and ARM64 to `ubuntu-24.04-arm`, then creates the final manifest.

## Decision

Use a repository-owned native matrix with local checkout context, per-platform digest pushes, and
a manifest merge job. Docker's official reusable builder was attempted first, but its ARM64 job
failed to resolve the remote Git context with `unknown API capability source.git.checksum`.
The local context keeps native parallelism while removing that compatibility boundary.

## Repository Mapping

- Image: `ghcr.io/${{ github.repository }}`.
- Platforms: `linux/amd64,linux/arm64`.
- Cache: enabled in `max` mode with one scope per architecture.
- PR: local checkout and build, no push, no registry secret.
- Publish: local checkout, GHCR authentication from `GITHUB_TOKEN`, and push by digest.
- Finalize: download both digest artifacts and create the tagged multi-platform manifest only after
  both builds succeed.
