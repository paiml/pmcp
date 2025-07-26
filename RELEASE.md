# Release Process

This document describes the release process for pmcp.

## Pre-release Checklist

- [ ] Update version in `Cargo.toml`
- [ ] Update version in `VERSION` file
- [ ] Update `CHANGELOG.md` with release notes
- [ ] Run `make quality-gate` and ensure all checks pass
- [ ] Run `make coverage-ci` and ensure coverage > 80%
- [ ] Run `make bench` and check for performance regressions
- [ ] Update README.md if needed
- [ ] Ensure CI is passing on main branch
- [ ] Commit all changes

## Release Steps

### Automated Release (Recommended)

For different types of releases, use the appropriate make command:

1. **Patch Release** (bug fixes, documentation)
   ```bash
   make release-patch
   git push origin main --tags
   ```

2. **Minor Release** (new features, backwards compatible)
   ```bash
   make release-minor
   git push origin main --tags
   ```

3. **Major Release** (breaking changes)
   ```bash
   make release-major
   git push origin main --tags
   ```

### Manual Release Process

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

3. **CI/CD will automatically:**
   - Run full test suite and quality checks
   - Create a GitHub Release with changelog
   - Publish to crates.io (requires CARGO_REGISTRY_TOKEN secret)
   - Generate coverage report

4. **Verify the release**
   - Check https://github.com/paiml/pmcp/releases for the new release
   - Check https://crates.io/crates/pmcp for the new version
   - Check https://docs.rs/pmcp for updated documentation

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