#!/bin/bash
# release.sh - Script to create releases for Open Very Fast Trace

set -e

if [ -z "$1" ]; then
    echo "Usage: $0 <version>"
    echo "Example: $0 0.1.0"
    exit 1
fi

VERSION="$1"
TAG="v$VERSION"

echo "🚀 Creating release $TAG"

# Ensure we're on main branch and up to date
git checkout main
git pull origin main

# Update version in Cargo.toml files
echo "📝 Updating version numbers..."
sed -i.bak "s/^version = \".*\"/version = \"$VERSION\"/" ovft-core/Cargo.toml
sed -i.bak "s/^version = \".*\"/version = \"$VERSION\"/" cargo-ovft/Cargo.toml
sed -i.bak "s/^version = \".*\"/version = \"$VERSION\"/" ovft-example/Cargo.toml

# Clean up backup files
rm -f ovft-core/Cargo.toml.bak cargo-ovft/Cargo.toml.bak ovft-example/Cargo.toml.bak

# Test that everything builds
echo "🔨 Testing build..."
cargo build --workspace
cargo test --workspace

# Commit version bump
git add .
git commit -m "Bump version to $VERSION"

# Create and push tag
echo "🏷️  Creating tag $TAG..."
git tag -a "$TAG" -m "Release $TAG"
git push origin main
git push origin "$TAG"

echo "✅ Release $TAG created successfully!"
echo "🔗 GitHub will automatically build and publish the release."
echo "📦 Check the Actions tab for build progress: https://github.com/jFiedler24/open-very-fast-trace/actions"
