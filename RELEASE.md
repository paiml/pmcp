# Release Process

This document describes the release process for pmcp.

## Pre-release Checklist

- [ ] Update version in `Cargo.toml`
- [ ] Update `CHANGELOG.md` with release notes
- [ ] Run `make check` and ensure all checks pass
- [ ] Run `make coverage-ci` and ensure coverage > 80%
- [ ] Run `make bench` and check for performance regressions
- [ ] Update README.md if needed
- [ ] Commit all changes

## Release Steps

1. **Create a release commit**
   ```bash
   git add -A
   git commit -m "chore: release v0.1.0"
   ```

2. **Create and push tag**
   ```bash
   git tag -a v0.1.0 -m "Release version 0.1.0"
   git push origin main
   git push origin v0.1.0
   ```

3. **Publish to crates.io**
   ```bash
   cargo publish --dry-run
   cargo publish
   ```

4. **Create GitHub Release**
   - Go to https://github.com/paiml/pmcp/releases
   - Click "Draft a new release"
   - Select the tag `v0.1.0`
   - Title: `v0.1.0`
   - Copy the CHANGELOG.md content for this version
   - Publish release

## Post-release

1. **Update version for development**
   ```bash
   # Update Cargo.toml version to 0.1.1-dev
   # Add new [Unreleased] section to CHANGELOG.md
   git add -A
   git commit -m "chore: bump version to 0.1.1-dev"
   git push origin main
   ```

## Version Numbering

We follow [Semantic Versioning](https://semver.org/):
- MAJOR version for incompatible API changes
- MINOR version for backwards-compatible functionality additions
- PATCH version for backwards-compatible bug fixes

## Release Frequency

- Patch releases: as needed for bug fixes
- Minor releases: monthly or when significant features are ready
- Major releases: only when breaking changes are necessary