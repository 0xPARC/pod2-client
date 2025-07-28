#!/bin/bash

# Version bump script for POD2 Client
# Updates version across all Tauri configuration files

set -e  # Exit on any error

# Check if version argument is provided
if [ $# -eq 0 ]; then
    echo "Usage: $0 <version>"
    echo "Example: $0 0.0.3"
    exit 1
fi

NEW_VERSION="$1"

# Validate version format (basic semver check)
if ! [[ "$NEW_VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.-]+)?(\+[a-zA-Z0-9.-]+)?$ ]]; then
    echo "Error: Invalid version format. Expected semantic version (e.g., 1.0.0, 0.1.0-beta, 1.2.3+build)"
    exit 1
fi

echo "🔄 Bumping version to $NEW_VERSION..."

# Get the project root directory (assuming script is in scripts/ subdirectory)
PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CLIENT_DIR="$PROJECT_ROOT/apps/client"

# Files to update
TAURI_CONF="$CLIENT_DIR/src-tauri/tauri.conf.json"
CARGO_TOML="$CLIENT_DIR/src-tauri/Cargo.toml"
PACKAGE_JSON="$CLIENT_DIR/package.json"

# Check if all files exist
for file in "$TAURI_CONF" "$CARGO_TOML" "$PACKAGE_JSON"; do
    if [ ! -f "$file" ]; then
        echo "Error: File not found: $file"
        exit 1
    fi
done

echo "📝 Updating configuration files..."

# Update tauri.conf.json
echo "  → Updating tauri.conf.json"
if command -v jq > /dev/null; then
    # Use jq if available for safer JSON manipulation
    jq --arg version "$NEW_VERSION" '.version = $version' "$TAURI_CONF" > "$TAURI_CONF.tmp" && mv "$TAURI_CONF.tmp" "$TAURI_CONF"
else
    # Fallback to sed
    sed -i.bak "s/\"version\": \"[^\"]*\"/\"version\": \"$NEW_VERSION\"/" "$TAURI_CONF" && rm "$TAURI_CONF.bak"
fi

# Update Cargo.toml
echo "  → Updating Cargo.toml"
sed -i.bak "s/^version = \"[^\"]*\"/version = \"$NEW_VERSION\"/" "$CARGO_TOML" && rm "$CARGO_TOML.bak"

# Update package.json
echo "  → Updating package.json"
if command -v jq > /dev/null; then
    # Use jq if available for safer JSON manipulation
    jq --arg version "$NEW_VERSION" '.version = $version' "$PACKAGE_JSON" > "$PACKAGE_JSON.tmp" && mv "$PACKAGE_JSON.tmp" "$PACKAGE_JSON"
else
    # Fallback to sed
    sed -i.bak "s/\"version\": \"[^\"]*\"/\"version\": \"$NEW_VERSION\"/" "$PACKAGE_JSON" && rm "$PACKAGE_JSON.bak"
fi

echo "✅ Version bump complete!"
echo ""
echo "Updated files:"
echo "  - $TAURI_CONF"
echo "  - $CARGO_TOML" 
echo "  - $PACKAGE_JSON"
echo ""
echo "Next steps:"
echo "  git add -A"
echo "  git commit -m \"Bump version to $NEW_VERSION\""
echo "  git tag v$NEW_VERSION"