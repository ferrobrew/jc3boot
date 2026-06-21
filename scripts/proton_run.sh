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
# Overridable via env: STEAM_ROOT, JC3_APPID, PROTON_DIR, JC3_EXE, JC3_BUS_NAME,
# GAMESCOPE_OPTS.
# Any args passed to this script replace the default injector args
# (`--spawn <JC3_EXE>`), except `--gamescope` (or `--gamescope=<opts>`), which is
# pulled out and instead wraps the whole launch in gamescope -- its own nested
# compositor, which sidesteps the host XWayland focus race (the game won't need a
# focus-bounce off Steam to take input). Tune gamescope via GAMESCOPE_OPTS or the
# `--gamescope=<opts>` value (default `-f` for fullscreen).
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

# Args go to the injector, except --gamescope (pulled out to wrap the launch).
USE_GAMESCOPE=0
inj_args=()
for arg in "$@"; do
  case "$arg" in
    --gamescope) USE_GAMESCOPE=1 ;;
    --gamescope=*) USE_GAMESCOPE=1; GAMESCOPE_OPTS="${arg#--gamescope=}" ;;
    *) inj_args+=("$arg") ;;
  esac
done
if [ ${#inj_args[@]} -eq 0 ]; then
  inj_args=(--spawn "$GAME")
fi

# Container layout:
#   [gamescope ->] steam-run -> sniper -> launcher service -> proton -> injector
# The launcher service claims $BUS on the session bus and keeps the container
# (and the game's wineserver) reachable for the lifetime of the game. gamescope,
# when enabled, wraps the whole chain on the host so the game renders into its own
# nested compositor; injection is unchanged (the injector still spawns + injects
# the game inside this same wineserver).
launch=(
  steam-run env
  STEAM_COMPAT_CLIENT_INSTALL_PATH="$STEAM"
  STEAM_COMPAT_DATA_PATH="$STEAM/steamapps/compatdata/$APPID"
  STEAM_COMPAT_APP_ID="$APPID" SteamAppId="$APPID" SteamGameId="$APPID"
  "$SNIPER" --verb=waitforexitandrun --
  steam-runtime-launcher-service --bus-name="$BUS" --hint --
  "$PROTON/proton" waitforexitandrun "$INJECTOR" "${inj_args[@]}"
)

if [ "$USE_GAMESCOPE" -eq 1 ]; then
  command -v gamescope >/dev/null \
    || { echo "proton_run.sh: --gamescope given but 'gamescope' is not in PATH" >&2; exit 1; }
  # The NixOS gamescope wrapper carries cap_sys_nice/cap_setpcap and raises them
  # into the AMBIENT set (so the game can use realtime scheduling). Those propagate
  # into steam-run's bwrap, which then aborts with "Unexpected capabilities but not
  # setuid". Strip the ambient + inheritable caps with setpriv before entering the
  # sandbox (the game loses realtime priority, which is fine here).
  # GAMESCOPE_OPTS is intentionally word-split so multiple flags work.
  # shellcheck disable=SC2086
  exec gamescope ${GAMESCOPE_OPTS:--f} -- \
    setpriv --inh-caps=-all --ambient-caps=-all "${launch[@]}"
else
  exec "${launch[@]}"
fi
