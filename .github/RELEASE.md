# GitHub Actions & Release Process

This project uses GitHub Actions for automated testing, building, and releasing.

## 🔄 Continuous Integration

The CI pipeline runs on every push and pull request:

### **Test Suite** (`ci.yml`)
- ✅ **Multi-Rust testing** - Stable, Beta, Nightly
- ✅ **Code formatting** - `cargo fmt --check`
- ✅ **Linting** - `cargo clippy` with strict warnings
- ✅ **Unit tests** - All workspace crates
- ✅ **Integration tests** - Full end-to-end testing
- ✅ **Plugin testing** - Verify `cargo ovft` works
- ✅ **Security audit** - `cargo audit` for vulnerabilities
- ✅ **Code coverage** - Upload to Codecov

## 🚀 Release Process

### **Automatic Releases** (`release.yml`)

Triggered when you push a version tag:

```bash
git tag v0.1.0
git push origin v0.1.0
```

The release workflow:
1. **Creates GitHub Release** with automated changelog
2. **Builds cross-platform binaries**:
   - Linux (x86_64)
   - macOS (Intel + Apple Silicon)  
   - Windows (x86_64)
3. **Uploads release assets** (.tar.gz, .zip)
4. **Publishes to crates.io** (both `ovft-core` and `cargo-ovft`)

### **Manual Release Process**

Use the included script:

```bash
# Bump version and create release
./release.sh 0.1.0
```

This script:
- Updates version numbers in all `Cargo.toml` files
- Runs tests to ensure everything works
- Commits the version bump
- Creates and pushes the git tag
- Triggers the automated release workflow

## 🔧 Setup for Publishing

### **1. crates.io Token**

Add your crates.io API token to GitHub Secrets:

1. Go to [crates.io/me](https://crates.io/me) → API Tokens
2. Create new token with publish permissions
3. Go to GitHub → Settings → Secrets → Actions
4. Add secret: `CARGO_REGISTRY_TOKEN` = your token

### **2. GitHub Token**

The `GITHUB_TOKEN` is automatically provided by GitHub Actions.

## 📋 Release Checklist

Before creating a release:

- [ ] Update `CHANGELOG.md` with new features/fixes
- [ ] Run `cargo test --workspace` locally
- [ ] Run `cargo ovft --check` to verify requirements
- [ ] Update version numbers if not using the script
- [ ] Ensure all dependencies are properly declared

## 🏷️ Version Tags

Follow semantic versioning:

- `v0.1.0` - Major.Minor.Patch
- `v0.1.1` - Bug fixes
- `v0.2.0` - New features
- `v1.0.0` - Stable API

## 📦 Published Packages

After release, users can install:

```bash
# From crates.io (stable)
cargo install cargo-ovft

# From GitHub (latest)
cargo install --git https://github.com/jFiedler24/open-very-fast-trace cargo-ovft

# Download binary from GitHub Releases
# Extract and add to PATH
```

## 🔍 Monitoring

- **GitHub Actions**: Check workflow status in the Actions tab
- **crates.io**: Monitor download stats at [crates.io/crates/cargo-ovft](https://crates.io/crates/cargo-ovft)
- **Codecov**: View coverage reports (if configured)
