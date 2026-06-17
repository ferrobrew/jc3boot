#!/usr/bin/env bash
# Build the payload + injector for x86-64 Windows and run the injector inside
# the game's Proton prefix, so the injected process matches the environment the
# game actually runs in.
#
# Why this exists: plain wine (e.g. from nixpkgs) is missing APIs Just Cause 3
# needs (advapi32.SystemFunction036 / RtlGenRandom, cryptbase forwarding), so
# `xwin_run.sh` aborts. The game works because Steam runs it under Proton (a
# wine fork) inside the Steam Linux Runtime "sniper" container. We reuse that
# exact stack: the injector spawns the game itself, so both live in the same
# wineserver and cross-process injection works.
#
# On NixOS, pressure-vessel/bwrap needs an FHS environment (/usr/bin/true etc.)
# that the system doesn't provide, so the whole thing is wrapped in `steam-run`.
#
# Overridable via env: STEAM_ROOT, JC3_APPID, PROTON_DIR, JC3_EXE.
# Any args passed to this script replace the default injector args
# (`--spawn <JC3_EXE>`), e.g. to attach to an already-running game instead.
set -euo pipefail

DIR="$(cd "$(dirname "$0")/.." && pwd)"
STEAM="${STEAM_ROOT:-$HOME/.steam/steam}"
APPID="${JC3_APPID:-225540}"
PROTON="${PROTON_DIR:-$STEAM/steamapps/common/Proton - Experimental}"
GAME="${JC3_EXE:-$STEAM/steamapps/common/Just Cause 3/JustCause3.exe}"
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

exec steam-run env \
  STEAM_COMPAT_CLIENT_INSTALL_PATH="$STEAM" \
  STEAM_COMPAT_DATA_PATH="$STEAM/steamapps/compatdata/$APPID" \
  STEAM_COMPAT_APP_ID="$APPID" SteamAppId="$APPID" SteamGameId="$APPID" \
  "$SNIPER" --verb=waitforexitandrun -- \
  "$PROTON/proton" waitforexitandrun "$INJECTOR" "${inj_args[@]}"
