#!/usr/bin/env bash
# Management script for GitHub Actions self-hosted runners using Lima
#
# Usage: ./manage-runners.sh <command> [runner-name]
#
# Commands:
#   create <name>     - Create and start a new Lima VM for a runner
#   start <name>      - Start an existing runner VM
#   stop <name>       - Stop a runner VM
#   shell <name>      - Open shell in runner VM
#   status            - Show status of all runner VMs
#   logs <name>       - Show runner service logs
#   delete <name>     - Delete a runner VM
#   list              - List all Lima VMs

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
LIMA_CONFIG="$REPO_ROOT/.github/lima/gh-runner.yaml"

command="${1:-}"
runner_name="${2:-}"

usage() {
  cat <<EOF
Usage: $0 <command> [runner-name]

Commands:
  create <name>     Create and start a new Lima VM for a runner
                    Example: $0 create gh-runner-1

  start <name>      Start an existing runner VM
                    Example: $0 start gh-runner-1

  stop <name>       Stop a runner VM
                    Example: $0 stop gh-runner-1

  shell <name>      Open shell in runner VM
                    Example: $0 shell gh-runner-1

  status            Show status of all runner VMs

  logs <name>       Show runner service logs (requires runner to be configured)
                    Example: $0 logs gh-runner-1

  delete <name>     Delete a runner VM (WARNING: destroys all data)
                    Example: $0 delete gh-runner-1

  list              List all Lima VMs

Setup Instructions:
  1. Get a registration token from GitHub:
     Go to: https://github.com/YOUR_ORG/YOUR_REPO/settings/actions/runners/new
     Select Linux and ARM64, then copy the registration token

  2. Create two runner VMs:
     $0 create gh-runner-1
     $0 create gh-runner-2

  3. Configure each runner (inside the VM):
     $0 shell gh-runner-1
     # Inside VM:
     git clone https://github.com/YOUR_ORG/YOUR_REPO.git
     cd YOUR_REPO
     ./.github/scripts/setup-runner.sh <REG_TOKEN> gh-runner-1
     exit

  4. Repeat for gh-runner-2

  5. Check status:
     $0 status

EOF
}

case "$command" in
  create)
    if [ -z "$runner_name" ]; then
      echo "Error: Runner name required"
      echo "Example: $0 create gh-runner-1"
      exit 1
    fi

    echo "Creating Lima VM: $runner_name"
    # Mount the .github/scripts directory read-only for easy access
    limactl start --name="$runner_name" --yes \
      --mount-type=9p \
      --mount="$REPO_ROOT/.github/scripts" \
      "$LIMA_CONFIG"
    echo ""
    echo "✅ VM created successfully!"
    echo ""
    echo "Next steps:"
    echo "  1. Shell into the VM: $0 shell $runner_name"
    echo "  2. Clone the repository and run setup-runner.sh"
    ;;

  start)
    if [ -z "$runner_name" ]; then
      echo "Error: Runner name required"
      echo "Example: $0 start gh-runner-1"
      exit 1
    fi

    echo "Starting Lima VM: $runner_name"
    limactl start "$runner_name"
    ;;

  stop)
    if [ -z "$runner_name" ]; then
      echo "Error: Runner name required"
      echo "Example: $0 stop gh-runner-1"
      exit 1
    fi

    echo "Stopping Lima VM: $runner_name"
    limactl stop "$runner_name"
    ;;

  shell)
    if [ -z "$runner_name" ]; then
      echo "Error: Runner name required"
      echo "Example: $0 shell gh-runner-1"
      exit 1
    fi

    echo "Opening shell in Lima VM: $runner_name"
    limactl shell "$runner_name"
    ;;

  status)
    echo "Lima VM Status:"
    limactl list
    ;;

  logs)
    if [ -z "$runner_name" ]; then
      echo "Error: Runner name required"
      echo "Example: $0 logs gh-runner-1"
      exit 1
    fi

    echo "Fetching logs for runner: $runner_name"
    echo "Note: This assumes the runner has been configured with setup-runner.sh"
    echo ""

    # Try to get the service name (format: actions.runner.OWNER.REPO.RUNNER_NAME)
    # This will vary based on the actual repository
    limactl shell "$runner_name" sudo journalctl -u "actions.runner.*${runner_name}.service" -n 50 --no-pager
    ;;

  delete)
    if [ -z "$runner_name" ]; then
      echo "Error: Runner name required"
      echo "Example: $0 delete gh-runner-1"
      exit 1
    fi

    echo "⚠️  WARNING: This will permanently delete the VM and all its data!"
    echo "VM name: $runner_name"
    read -r -p "Are you sure? (yes/no): " confirm

    if [ "$confirm" = "yes" ]; then
      echo "Deleting Lima VM: $runner_name"
      limactl delete "$runner_name"
      echo "✅ VM deleted"
    else
      echo "Cancelled"
    fi
    ;;

  list)
    echo "All Lima VMs:"
    limactl list
    ;;

  ""|help|--help|-h)
    usage
    ;;

  *)
    echo "Error: Unknown command '$command'"
    echo ""
    usage
    exit 1
    ;;
esac
