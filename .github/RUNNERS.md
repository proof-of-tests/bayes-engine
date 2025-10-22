# Self-Hosted GitHub Runners Setup

This document describes how to set up and manage self-hosted GitHub Actions runners using Lima (Linux-on-Mac) for
aarch64-linux.

## Overview

We use Lima to create lightweight Linux VMs on macOS that run GitHub Actions self-hosted runners. This gives us:

- **Native ARM64 Linux environment** for accurate testing
- **Nix support** for reproducible builds
- **Local control** over runner configuration
- **Cost savings** compared to GitHub-hosted runners for intensive workloads

## Security Warning

**IMPORTANT**: GitHub strongly recommends using self-hosted runners only with **private repositories**. Self-hosted
runners used with public repositories can be a security risk:

- Forks of public repositories can potentially run dangerous code on your runner machines via pull requests
- Malicious actors could execute arbitrary code in your infrastructure
- Secrets and environment variables may be exposed to untrusted code

For public repositories, always use GitHub-hosted runners unless you have specific isolation and security measures in
place. See
[GitHub's security hardening guide](https://docs.github.com/en/actions/security-for-github-actions/security-guides/security-hardening-for-github-actions#hardening-for-self-hosted-runners)
for more information.

## Architecture

```
macOS Host
  ├── Lima VM: gh-runner-1 (aarch64-linux)
  │   └── GitHub Actions Runner Service
  └── Lima VM: gh-runner-2 (aarch64-linux)
      └── GitHub Actions Runner Service
```

Each Lima VM:

- Runs Ubuntu 24.04 ARM64
- Has Nix installed with flakes enabled
- Runs GitHub Actions runner as a systemd service
- Has 4 CPUs, 8GB RAM, 50GB disk (configurable)

## Prerequisites

- macOS with Apple Silicon (M1/M2/M3)
- Lima installed (`brew install lima` or via Nix)
- Access to repository settings to generate runner registration tokens
- This repository cloned locally

## Quick Start

### 1. Create Runner VMs

```bash
# Create first runner
./.github/scripts/manage-runners.sh create gh-runner-1

# Create second runner
./.github/scripts/manage-runners.sh create gh-runner-2
```

This will:

- Create Ubuntu 24.04 ARM64 VMs
- Install Nix with flakes support
- Set up system dependencies
- Start the VMs

### 2. Configure Runners

For each runner, you need to register it with GitHub using a registration token.

#### Get Registration Token

1. Go to your repository on GitHub
2. Navigate to **Settings → Actions → Runners → New self-hosted runner**
3. Select **Linux** and **ARM64** architecture
4. Copy the **registration token** shown in the configuration commands (starts with `A` and expires after 1 hour)

#### Configure First Runner

```bash
limactl shell gh-runner-1 "~/setup-runner.sh <REG_TOKEN>"
```

Replace `<REG_TOKEN>` with the token you copied from GitHub. The script is automatically created in the VM's home
directory during provisioning.

#### Configure Second Runner

Get a new registration token from GitHub (or reuse the previous one if still valid), then:

```bash
limactl shell gh-runner-2 "~/setup-runner.sh <REG_TOKEN>"
```

**Note**: Registration tokens expire after 1 hour. If your token expires, generate a new one from the GitHub UI.

### 3. Verify Runners

```bash
# Check VM status
./.github/scripts/manage-runners.sh status

# Check runner logs
./.github/scripts/manage-runners.sh logs gh-runner-1
./.github/scripts/manage-runners.sh logs gh-runner-2
```

On GitHub:

1. Go to your repository
2. Navigate to Settings → Actions → Runners
3. You should see `gh-runner-1` and `gh-runner-2` with status "Idle"

### 4. Update CI Workflows (Optional)

To use self-hosted runners in your workflows, update `.github/workflows/ci.yml`:

```yaml
jobs:
  nix-checks:
    name: Nix Flake Checks
    runs-on: [self-hosted, Linux, ARM64]  # Changed from ubuntu-latest
    steps:
      # ... rest of the job
```

**Note:** Nix is already installed in the runner VMs, so you don't need the "Install Nix" step.

## Management Commands

The `manage-runners.sh` script provides convenient commands:

```bash
# Create a new runner VM
./.github/scripts/manage-runners.sh create <name>

# Start a stopped VM
./.github/scripts/manage-runners.sh start <name>

# Stop a running VM
./.github/scripts/manage-runners.sh stop <name>

# Open shell in VM
./.github/scripts/manage-runners.sh shell <name>

# Show status of all VMs
./.github/scripts/manage-runners.sh status

# Show runner service logs
./.github/scripts/manage-runners.sh logs <name>

# Delete a VM (WARNING: destroys all data)
./.github/scripts/manage-runners.sh delete <name>

# List all Lima VMs
./.github/scripts/manage-runners.sh list
```

## Configuration

### Lima VM Configuration

Edit `.github/lima/gh-runner.yaml` to customize:

- **CPUs**: `cpus: 4` (adjust based on your Mac)
- **Memory**: `memory: "8GiB"` (adjust based on your Mac)
- **Disk**: `disk: "50GiB"` (adjust based on needs)

After changing, recreate VMs:

```bash
./.github/scripts/manage-runners.sh delete gh-runner-1
./.github/scripts/manage-runners.sh create gh-runner-1
# (re-run setup-runner.sh)
```

### Runner Labels

Runners are configured with labels:

- `self-hosted`
- `Linux`
- `ARM64`

To add custom labels, modify `setup-runner.sh` line:

```bash
--labels "self-hosted,Linux,ARM64,custom-label"
```

## Troubleshooting

### Runner Not Starting

Check the service status:

```bash
limactl shell gh-runner-1
sudo systemctl status actions.runner.*.service
sudo journalctl -u actions.runner.*.service -n 100
```

### Nix Command Not Found

Source Nix profile:

```bash
limactl shell gh-runner-1
source /nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh
nix --version
```

### VM Won't Start

Check Lima logs:

```bash
limactl list
cat ~/.lima/gh-runner-1/serial.log
```

Delete and recreate if needed:

```bash
limactl delete gh-runner-1
./.github/scripts/manage-runners.sh create gh-runner-1
```

### Runner Shows as Offline

1. Check VM is running: `limactl list`
2. Check service status: `./.github/scripts/manage-runners.sh logs gh-runner-1`
3. Restart service:
   ```bash
   limactl shell gh-runner-1
   cd ~/actions-runner-gh-runner-1
   sudo ./svc.sh restart
   ```

### Build Fails on Self-Hosted Runner

1. Ensure Nix is available (should be pre-installed)
2. Check disk space: `limactl shell gh-runner-1 df -h`
3. Check runner logs for errors

## Maintenance

### Updating Runner Software

GitHub Actions runner auto-updates, but to manually update:

```bash
limactl shell gh-runner-1
cd ~/actions-runner-gh-runner-1
sudo ./svc.sh stop
./config.sh remove --token <YOUR_TOKEN>
rm -rf ~/actions-runner-gh-runner-1
# Re-run setup-runner.sh
```

### Updating System Packages

```bash
limactl shell gh-runner-1
sudo apt update && sudo apt upgrade -y
```

### Cleaning Up Disk Space

```bash
limactl shell gh-runner-1

# Clean Nix store
nix-collect-garbage -d

# Clean apt cache
sudo apt clean

# Remove old runner work files
cd ~/actions-runner-gh-runner-1/_work
rm -rf */  # Be careful! This removes all workflow artifacts
```

## Security Considerations

1. **Token Security**

   - Use registration tokens (expire after 1 hour) instead of PATs when possible
   - Never commit tokens to the repository
   - Rotate PATs regularly

2. **VM Isolation**

   - Each runner runs in its own isolated VM
   - Runners have no write access to host filesystem by default
   - Consider using separate VMs for untrusted workflows

3. **Updates**

   - Keep Lima updated: `brew upgrade lima`
   - Keep Ubuntu updated: `apt update && apt upgrade`
   - Keep Nix updated: `nix upgrade-nix`

4. **Secrets**

   - Repository secrets are available to self-hosted runners
   - Be cautious with runners in public repositories
   - Consider using environment protection rules

## Cost Comparison

**GitHub-Hosted Runners (ubuntu-latest):**

- Free for public repos (2,000 minutes/month)
- $0.008/minute for private repos

**Self-Hosted Runners:**

- No GitHub charges
- Local compute costs (electricity, hardware wear)
- Better for intensive builds (Nix, large compilations)

## Advanced Usage

### Running Multiple Jobs in Parallel

Both runners can execute jobs simultaneously. GitHub Actions will automatically distribute jobs across available
runners.

### Custom Runner Groups

For organization-level runner management, see
[GitHub's documentation](https://docs.github.com/en/actions/hosting-your-own-runners/managing-self-hosted-runners/managing-access-to-self-hosted-runners-using-groups).

### Auto-Starting Runners on Boot

Lima VMs can auto-start when you log in:

```bash
# Add to ~/.zshrc or ~/.bashrc
limactl start gh-runner-1 &
limactl start gh-runner-2 &
```

Or use launchd (macOS):

```xml
<!-- ~/Library/LaunchAgents/com.github.runner.plist -->
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.github.runner</string>
    <key>ProgramArguments</key>
    <array>
        <string>/opt/homebrew/bin/limactl</string>
        <string>start</string>
        <string>gh-runner-1</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
</dict>
</plist>
```

## Resources

- [Lima Documentation](https://lima-vm.io/docs/)
- [GitHub Actions Self-Hosted Runners](https://docs.github.com/en/actions/hosting-your-own-runners)
- [Nix Flakes](https://nixos.wiki/wiki/Flakes)
- [CloudFlare Workers CI/CD](https://developers.cloudflare.com/workers/ci-cd/)

## Support

If you encounter issues:

1. Check the troubleshooting section above
2. Review runner logs: `./.github/scripts/manage-runners.sh logs <name>`
3. Check Lima logs: `~/.lima/<name>/serial.log`
4. Open an issue in the repository
