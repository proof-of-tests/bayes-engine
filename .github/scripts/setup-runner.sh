#!/usr/bin/env bash
# Setup script for GitHub Actions self-hosted runner
# This script should be run inside the Lima VM
#
# Usage: ./setup-runner.sh <GITHUB_TOKEN> <RUNNER_NAME>
#   GITHUB_TOKEN: Personal access token with 'repo' scope (or use a registration token from GitHub API)
#   RUNNER_NAME: Name for this runner (e.g., gh-runner-1, gh-runner-2)

set -euo pipefail

if [ $# -lt 2 ]; then
  echo "Usage: $0 <GITHUB_TOKEN> <RUNNER_NAME>"
  echo ""
  echo "Example: $0 ghp_xxxxxxxxxxxx gh-runner-1"
  echo ""
  echo "To get a registration token, visit:"
  echo "https://github.com/YOUR_ORG/YOUR_REPO/settings/actions/runners/new"
  exit 1
fi

GITHUB_TOKEN="$1"
RUNNER_NAME="$2"

# Get repository from git remote (assumes we're in repo directory)
if [ -d .git ]; then
  REPO_URL=$(git config --get remote.origin.url)
  # Extract owner/repo from URL
  if [[ $REPO_URL =~ github.com[:/](.+/.+)(\.git)?$ ]]; then
    REPO_FULL_NAME="${BASH_REMATCH[1]%.git}"
  else
    echo "Error: Could not extract repository from git remote"
    exit 1
  fi
else
  echo "Error: Not in a git repository. Please provide REPO_FULL_NAME environment variable."
  echo "Example: REPO_FULL_NAME=owner/repo $0 ..."
  exit 1
fi

echo "Setting up GitHub Actions runner: $RUNNER_NAME"
echo "Repository: $REPO_FULL_NAME"

# Download the latest runner package for Linux ARM64
RUNNER_VERSION="2.321.0"  # Update this to latest version
RUNNER_ARCH="linux-arm64"
RUNNER_URL="https://github.com/actions/runner/releases/download/v${RUNNER_VERSION}/actions-runner-${RUNNER_ARCH}-${RUNNER_VERSION}.tar.gz"

# Create runner directory
RUNNER_DIR="$HOME/actions-runner-$RUNNER_NAME"
mkdir -p "$RUNNER_DIR"
cd "$RUNNER_DIR"

# Download and extract runner
echo "Downloading runner..."
curl -o actions-runner.tar.gz -L "$RUNNER_URL"
tar xzf actions-runner.tar.gz
rm actions-runner.tar.gz

# Get a registration token from GitHub API
echo "Getting registration token from GitHub..."
REG_TOKEN=$(curl -sS -X POST \
  -H "Accept: application/vnd.github.v3+json" \
  -H "Authorization: token ${GITHUB_TOKEN}" \
  "https://api.github.com/repos/${REPO_FULL_NAME}/actions/runners/registration-token" \
  | jq -r '.token')

if [ -z "$REG_TOKEN" ] || [ "$REG_TOKEN" = "null" ]; then
  echo "Error: Failed to get registration token. Check your GitHub token permissions."
  exit 1
fi

# Configure the runner
echo "Configuring runner..."
./config.sh \
  --url "https://github.com/${REPO_FULL_NAME}" \
  --token "$REG_TOKEN" \
  --name "$RUNNER_NAME" \
  --labels "self-hosted,Linux,ARM64" \
  --work "_work" \
  --unattended \
  --replace

# Install as a service
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
echo "  journalctl -u actions.runner.${REPO_FULL_NAME/\//.}.${RUNNER_NAME}.service -f"
echo ""
echo "To stop the runner:"
echo "  sudo ./svc.sh stop"
echo ""
echo "To uninstall the runner:"
echo "  sudo ./svc.sh uninstall"
echo "  ./config.sh remove --token <GITHUB_TOKEN>"
