#!/usr/bin/env bash
# Build the payload + injector for x86-64 Windows and launch Just Cause 3 under
# Proton with a Steam-runtime launcher service in the same container, so other
# tools (e.g. jc3vrs) can inject into the running game later.
#
# Why the launcher service: injection needs the injector and the game to share
# one wineserver. A wineserver is per-WINEPREFIX, but its socket lives in
# /tmp/.wine-$UID, and pressure-vessel gives every `proton run` a *private*
# /tmp tmpfs — so a second, separate Proton session can never see this game's
# processes. The fix is to run a `steam-runtime-launcher-service` as this
# container's top-level process; the game runs inside it, and later injections
# are sent into this same container (shared /tmp -> shared wineserver) via
# `steam-runtime-launch-client` (see jc3vrs/scripts/proton_run.sh).
#
# The service wraps the game command and exits with it (--stop-on-exit is the
# default), so the container lives exactly as long as the game.
#
# On NixOS the whole thing runs under `steam-run` because pressure-vessel/bwrap
# needs an FHS env (/usr/bin/true etc.) the system doesn't provide.
#
# Overridable via env: STEAM_ROOT, JC3_APPID, PROTON_DIR, JC3_EXE, JC3_BUS_NAME.
# Any args passed to this script replace the default injector args
# (`--spawn <JC3_EXE>`).
set -euo pipefail

DIR="$(cd "$(dirname "$0")/.." && pwd)"
STEAM="${STEAM_ROOT:-$HOME/.steam/steam}"
APPID="${JC3_APPID:-225540}"
PROTON="${PROTON_DIR:-$STEAM/steamapps/common/Proton - Experimental}"
GAME="${JC3_EXE:-$STEAM/steamapps/common/Just Cause 3/JustCause3.exe}"
BUS="${JC3_BUS_NAME:-com.jc3vrs.JustCause3}"
SNIPER="$STEAM/steamapps/common/SteamLinuxRuntime_sniper/_v2-entry-point"
INJECTOR="$DIR/target/x86_64-pc-windows-msvc/debug/jc3boot_injector.exe"

# Build with the cross toolchain (cargo-xwin lives in the nix shell, not under
# steam-run/Proton — keep the two invocations separate).
nix-shell "$DIR/shell.nix" --run \
  "cd '$DIR' && cargo xwin build --xwin-cache-dir .xwin --target x86_64-pc-windows-msvc -p jc3boot_payload -p jc3boot_injector"

inj_args=("$@")
if [ ${#inj_args[@]} -eq 0 ]; then
  inj_args=(--spawn "$GAME")
fi

# Container layout:
#   steam-run -> sniper entry point -> launcher service -> proton -> injector
# The launcher service claims $BUS on the session bus and keeps the container
# (and the game's wineserver) reachable for the lifetime of the game.
exec steam-run env \
  STEAM_COMPAT_CLIENT_INSTALL_PATH="$STEAM" \
  STEAM_COMPAT_DATA_PATH="$STEAM/steamapps/compatdata/$APPID" \
  STEAM_COMPAT_APP_ID="$APPID" SteamAppId="$APPID" SteamGameId="$APPID" \
  "$SNIPER" --verb=waitforexitandrun -- \
  steam-runtime-launcher-service --bus-name="$BUS" --hint -- \
  "$PROTON/proton" waitforexitandrun "$INJECTOR" "${inj_args[@]}"
