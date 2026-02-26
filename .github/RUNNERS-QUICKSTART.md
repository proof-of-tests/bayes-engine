# GitHub Runners Quick Start Guide

Get your self-hosted GitHub Actions runners up and running in 5 minutes!

## ⚠️ Security Warning

**Self-hosted runners should only be used with private repositories.** Using them with public repositories can allow
malicious actors to execute arbitrary code on your infrastructure via pull requests. See
[RUNNERS.md](.github/RUNNERS.md#security-warning) for details.

## Prerequisites

- macOS with Apple Silicon
- Lima installed
- Access to repository settings on GitHub

## Step-by-Step Setup

### 1. Get Registration Token from GitHub

1. Go to your repository on GitHub
2. Navigate to **Settings → Actions → Runners → New self-hosted runner**
3. Select **Linux** and **ARM64**
4. Copy the **registration token** from the configuration commands
5. Keep this token handy (expires after 1 hour)

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
limactl shell gh-runner-1 "~/setup-runner.sh REG_TOKEN"
```

Replace `REG_TOKEN` with your actual registration token. The setup script is automatically created in the VM during
provisioning.

### 4. Configure Second Runner

You can reuse the same registration token if it hasn't expired, or get a new one from GitHub.

```bash
limactl shell gh-runner-2 "~/setup-runner.sh REG_TOKEN"
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
./config.sh remove --token REG_TOKEN
# Get a new registration token from GitHub
# Then re-run setup-runner.sh with the new token
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
