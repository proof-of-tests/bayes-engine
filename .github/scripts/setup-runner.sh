#!/usr/bin/env bash
# Setup script for GitHub Actions self-hosted runner
# This script should be run inside the Lima VM
#
# Usage: ./setup-runner.sh <REG_TOKEN> <RUNNER_NAME>
#   REG_TOKEN: Registration token from GitHub (expires after 1 hour)
#   RUNNER_NAME: Name for this runner (e.g., gh-runner-1, gh-runner-2)
#
# To get a registration token:
#   1. Go to: https://github.com/OWNER/REPO/settings/actions/runners/new
#   2. Select "Linux" and "ARM64"
#   3. Copy the token from the configuration commands

set -euo pipefail

if [ $# -lt 2 ]; then
  echo "Usage: $0 <REG_TOKEN> <RUNNER_NAME>"
  echo ""
  echo "Example: $0 ABCD1234EXAMPLE gh-runner-1"
  echo ""
  echo "To get a registration token, visit:"
  echo "  https://github.com/OWNER/REPO/settings/actions/runners/new"
  echo "  (Select Linux and ARM64, then copy the token)"
  exit 1
fi

REG_TOKEN="$1"
RUNNER_NAME="$2"

# Repository URL
REPO_URL="https://github.com/proof-of-tests/bayes-engine"

echo "Setting up GitHub Actions runner: $RUNNER_NAME"
echo "Repository: $REPO_URL"
echo ""

# Download the latest runner package for Linux ARM64
RUNNER_VERSION="2.329.0"
RUNNER_ARCH="linux-arm64"
RUNNER_PACKAGE="actions-runner-${RUNNER_ARCH}-${RUNNER_VERSION}.tar.gz"
RUNNER_URL="https://github.com/actions/runner/releases/download/v${RUNNER_VERSION}/${RUNNER_PACKAGE}"
RUNNER_HASH="56768348b3d643a6a29d4ad71e9bdae0dc0ef1eb01afe0f7a8ee097b039bfaaf"

# Create a folder
RUNNER_DIR="$HOME/actions-runner-$RUNNER_NAME"
mkdir -p "$RUNNER_DIR"
cd "$RUNNER_DIR"

# Download the latest runner package
echo "Downloading the latest runner package..."
curl -o "${RUNNER_PACKAGE}" -L "${RUNNER_URL}"

# Optional: Validate the hash
echo "Validating hash..."
if ! echo "${RUNNER_HASH}  ${RUNNER_PACKAGE}" | shasum -a 256 -c; then
  echo "Error: Hash validation failed. The downloaded file may be corrupted."
  exit 1
fi

# Extract the installer
echo "Extracting installer..."
tar xzf "./${RUNNER_PACKAGE}"
rm "${RUNNER_PACKAGE}"

# Create the runner and start the configuration experience
echo ""
echo "Configuring runner..."
./config.sh \
  --url "$REPO_URL" \
  --token "$REG_TOKEN" \
  --name "$RUNNER_NAME" \
  --labels "self-hosted,Linux,ARM64" \
  --unattended

# Install and start as a systemd service (instead of ./run.sh)
echo ""
echo "Installing runner as systemd service..."
sudo ./svc.sh install
sudo ./svc.sh start

echo ""
echo "✅ GitHub Actions runner '$RUNNER_NAME' has been configured and started!"
echo ""
echo "To check status:"
echo "  sudo ./svc.sh status"
echo ""
echo "To view logs:"
echo "  journalctl -u actions.runner.proof-of-tests.bayes-engine.${RUNNER_NAME}.service -f"
echo ""
echo "To stop the runner:"
echo "  sudo ./svc.sh stop"
echo ""
echo "To uninstall the runner:"
echo "  sudo ./svc.sh uninstall"
echo "  ./config.sh remove --token <REG_TOKEN>"
