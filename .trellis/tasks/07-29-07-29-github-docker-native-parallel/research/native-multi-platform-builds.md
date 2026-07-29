# Native Multi-platform GitHub Builds

## Sources

- GitHub-hosted runners reference: `ubuntu-24.04-arm` is an ARM64 Linux runner available to public repositories.
- Docker multi-platform GitHub Actions guide: distributing builds avoids the significant QEMU runtime cost of building multiple platforms on one runner.
- Docker GitHub Builder reference: `build.yml@v1` defaults to `distribute: true`, maps AMD64 to `ubuntu-24.04` and ARM64 to `ubuntu-24.04-arm`, then creates the final manifest.

## Decision

Use Docker's official reusable builder instead of maintaining a custom digest artifact and manifest merge workflow. Keep separate PR and publish caller jobs so registry write permission remains unavailable to pull requests.

## Repository Mapping

- Image: `ghcr.io/${{ github.repository }}`.
- Platforms: `linux/amd64,linux/arm64`.
- Cache: enabled in `max` mode with a stable backend image scope.
- PR: image output, no push, no registry secret.
- Publish: image output, push enabled, GHCR authentication from `GITHUB_TOKEN`.
- Signing: automatic for publish; OIDC also signs cache entries.
