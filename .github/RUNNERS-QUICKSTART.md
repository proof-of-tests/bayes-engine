# GitHub Runners Quick Start Guide

Get your self-hosted GitHub Actions runners up and running in 5 minutes!

## Prerequisites

- macOS with Apple Silicon
- Lima installed
- GitHub Personal Access Token (PAT) with `repo` scope

## Step-by-Step Setup

### 1. Create GitHub Personal Access Token

1. Go to https://github.com/settings/tokens
2. Click "Generate new token" → "Generate new token (classic)"
3. Give it a name: "GitHub Runner Token"
4. Select scope: `repo` (full control of private repositories)
5. Click "Generate token"
6. **Copy the token** (you won't be able to see it again!)

### 2. Create Runner VMs

```bash
cd /path/to/bayes-engine

# Create first runner VM
./.github/scripts/manage-runners.sh create gh-runner-1

# Wait for it to finish (may take 2-3 minutes)
# Then create second runner
./.github/scripts/manage-runners.sh create gh-runner-2
```

### 3. Configure First Runner

```bash
# Open shell in first runner
./.github/scripts/manage-runners.sh shell gh-runner-1
```

Inside the VM:

```bash
# Clone the repository
git clone https://github.com/YOUR_USERNAME/bayes-engine.git
cd bayes-engine

# Run setup script (replace YOUR_TOKEN with your GitHub PAT)
./.github/scripts/setup-runner.sh ghp_YOUR_TOKEN_HERE gh-runner-1

# Wait for setup to complete, then exit
exit
```

### 4. Configure Second Runner

```bash
# Open shell in second runner
./.github/scripts/manage-runners.sh shell gh-runner-2
```

Inside the VM:

```bash
# Clone the repository
git clone https://github.com/YOUR_USERNAME/bayes-engine.git
cd bayes-engine

# Run setup script
./.github/scripts/setup-runner.sh ghp_YOUR_TOKEN_HERE gh-runner-2

# Exit
exit
```

### 5. Verify Runners

```bash
# Check VM status
./.github/scripts/manage-runners.sh status

# Should show both runners as "Running"
```

On GitHub:

1. Go to https://github.com/YOUR_USERNAME/bayes-engine/settings/actions/runners
2. You should see:
   - `gh-runner-1` - Status: Idle
   - `gh-runner-2` - Status: Idle

### 6. Test Runners (Optional)

Create a test workflow:

```yaml
# .github/workflows/test-runner.yml
name: Test Self-Hosted Runner

on: workflow_dispatch

jobs:
  test:
    runs-on: [self-hosted, Linux, ARM64]
    steps:
      - uses: actions/checkout@v5
      - name: Test runner
        run: |
          echo "Running on: $(uname -a)"
          nix --version
          echo "✅ Self-hosted runner works!"
```

Trigger it manually from GitHub Actions tab.

## Common Commands

```bash
# Check status
./.github/scripts/manage-runners.sh status

# View logs
./.github/scripts/manage-runners.sh logs gh-runner-1

# Shell into VM
./.github/scripts/manage-runners.sh shell gh-runner-1

# Stop a runner
./.github/scripts/manage-runners.sh stop gh-runner-1

# Start a runner
./.github/scripts/manage-runners.sh start gh-runner-1
```

## Troubleshooting

**Runner shows as offline:**

```bash
./.github/scripts/manage-runners.sh logs gh-runner-1
```

**VM won't start:**

```bash
limactl list
cat ~/.lima/gh-runner-1/serial.log
```

**Need to re-register runner:**

```bash
limactl shell gh-runner-1
cd ~/actions-runner-gh-runner-1
sudo ./svc.sh stop
./config.sh remove --token YOUR_TOKEN
# Then re-run setup-runner.sh
```

## Next Steps

- See [RUNNERS.md](.github/RUNNERS.md) for complete documentation
- Update CI workflows to use self-hosted runners
- Configure auto-start on boot (see RUNNERS.md)

## Getting Help

If you run into issues, check:

1. Runner logs: `./.github/scripts/manage-runners.sh logs <name>`
2. Lima logs: `~/.lima/<name>/serial.log`
3. Full documentation: [RUNNERS.md](.github/RUNNERS.md)
