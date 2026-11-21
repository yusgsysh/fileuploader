#!/usr/bin/env bash
set -euo pipefail

# Enable HTTP Basic Auth for a Podman Quadlet installation.
# Credentials are written outside the repository and never baked into the image.

usage() {
  echo "Usage: $0 [--system] [username] [password]" >&2
  echo "  Without a password, a random 32-character password is generated." >&2
  echo "  --system requires root and configures the rootful Quadlet unit." >&2
  exit 2
}

MODE="user"
if [ "${1:-}" = "--system" ]; then
  MODE="system"
  shift
fi
if [ "$#" -gt 2 ]; then
  usage
fi

USERNAME="${1:-admin}"
PASSWORD="${2:-}"

if [ "$MODE" = "system" ] && [ "$(id -u)" -ne 0 ]; then
  echo "--system must be run as root" >&2
  exit 1
fi
if [ -z "$USERNAME" ]; then
  echo "Username cannot be empty" >&2
  exit 1
fi
case "$USERNAME" in
  *:*|*$'\n'*|*$'\r'*)
    echo "Username cannot contain ':' or a newline" >&2
    exit 1
    ;;
esac
if [ -z "$PASSWORD" ]; then
  if command -v openssl >/dev/null 2>&1; then
    PASSWORD="$(openssl rand -hex 16)"
  else
    PASSWORD="$(od -An -N16 -tx1 /dev/urandom | tr -d ' \n')"
  fi
fi
case "$PASSWORD" in
  *$'\n'*|*$'\r'*)
    echo "Password cannot contain a newline" >&2
    exit 1
    ;;
esac

if [ "$MODE" = "system" ]; then
  AUTH_FILE="/etc/fileuploader-auth.env"
  UNIT_DIR="/etc/containers/systemd/fileuploader.container.d"
  SYSTEMCTL=(systemctl)
  SERVICE_LABEL="rootful"
else
  AUTH_FILE="${HOME}/.config/fileuploader-auth.env"
  UNIT_DIR="${HOME}/.config/containers/systemd/fileuploader.container.d"
  SYSTEMCTL=(systemctl --user)
  SERVICE_LABEL="rootless"
fi

umask 077
mkdir -p "$(dirname "$AUTH_FILE")" "$UNIT_DIR"
printf 'APP_BASIC_AUTH_USERNAME=%s\nAPP_BASIC_AUTH_PASSWORD=%s\n' "$USERNAME" "$PASSWORD" > "$AUTH_FILE"
chmod 600 "$AUTH_FILE"
cat > "$UNIT_DIR/10-auth.conf" <<EOF
[Container]
EnvironmentFile=$AUTH_FILE
EOF
chmod 600 "$UNIT_DIR/10-auth.conf"

"${SYSTEMCTL[@]}" daemon-reload
if "${SYSTEMCTL[@]}" list-unit-files fileuploader.service >/dev/null 2>&1; then
  "${SYSTEMCTL[@]}" restart fileuploader.service || true
fi

echo "HTTP Basic Auth configured for the ${SERVICE_LABEL} Quadlet."
echo "Username: ${USERNAME}"
echo "Password: ${PASSWORD}"
echo "Credentials file: ${AUTH_FILE} (mode 0600)"
echo "Restart/start fileuploader.service after the new image is available:"
if [ "$MODE" = "system" ]; then
  echo "  systemctl restart fileuploader.service"
else
  echo "  systemctl --user restart fileuploader.service"
fi
