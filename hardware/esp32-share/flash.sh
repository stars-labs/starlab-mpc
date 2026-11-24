#!/usr/bin/env bash
# One-command build & flash helper for Starlab Key (ESP32-S3 / ESP32-C3).
#
# Prerequisites:
#   cargo install espup espflash ldproxy
#   espup install                     # installs Xtensa + RISC-V ESP toolchains
#   source ~/export-esp.sh
#
# Usage:
#   ./flash.sh --board t-display-s3 --port /dev/ttyACM0
#   ./flash.sh --board xiao-c3 --port /dev/ttyACM0
#   ./flash.sh --board cyd-s3 --port /dev/ttyUSB0 --release-secure

set -euo pipefail

BOARD="t-display-s3"
PORT=""
SECURE_RELEASE=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --board) BOARD="$2"; shift 2 ;;
    --port) PORT="$2"; shift 2 ;;
    --release-secure) SECURE_RELEASE=1; shift ;;
    -h|--help)
      sed -n '2,15p' "$0"
      exit 0
      ;;
    *) echo "Unknown flag: $1" >&2; exit 1 ;;
  esac
done

case "$BOARD" in
  t-display-s3)
    TARGET="xtensa-esp32s3-espidf"
    FEATURE="board-t-display-s3"
    MCU="esp32s3"
    ;;
  cyd-s3)
    TARGET="xtensa-esp32s3-espidf"
    FEATURE="board-cyd-s3"
    MCU="esp32s3"
    ;;
  devkit-s3)
    TARGET="xtensa-esp32s3-espidf"
    FEATURE="board-devkit-s3"
    MCU="esp32s3"
    ;;
  xiao-c3)
    TARGET="riscv32imc-esp-espidf"
    FEATURE="board-xiao-c3"
    MCU="esp32c3"
    ;;
  *)
    echo "Unsupported board '$BOARD'. Choose: t-display-s3 | xiao-c3 | cyd-s3 | devkit-s3" >&2
    exit 1
    ;;
esac

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

if [[ "$SECURE_RELEASE" -eq 1 ]]; then
  echo "[!] Building HARDENED production image (Flash Encryption + Secure Boot V2)"
  export ESP_IDF_SDKCONFIG_DEFAULTS="sdkconfig.defaults;sdkconfig.release"
else
  export ESP_IDF_SDKCONFIG_DEFAULTS="sdkconfig.defaults"
fi

echo "==> Building starlab-esp32-share for $BOARD ($TARGET, feature=$FEATURE)..."
cargo +esp build --release --target "$TARGET" --no-default-features --features "$FEATURE"

ELF="target/${TARGET}/release/starlab-esp32-share"
FLASH_ARGS=(flash --chip "$MCU" --partition-table partitions.csv)
if [[ -n "$PORT" ]]; then
  FLASH_ARGS+=(--port "$PORT")
fi

echo "==> Flashing $ELF to $MCU..."
espflash "${FLASH_ARGS[@]}" "$ELF"
echo "==> Done! Connect at 115200 baud (NDJSON SLHW v1)."
