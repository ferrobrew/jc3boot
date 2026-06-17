#!/bin/sh
# Cross-compile the payload + injector for x86-64 Windows and run the injector
# under wine. The injector loads jc3boot_payload.dll from its own directory, so
# both crates build into the same target dir. Pass injector args (e.g. --spawn,
# a path to the game executable) through after the script name.
set -e
cargo xwin build --xwin-cache-dir .xwin --target x86_64-pc-windows-msvc -p jc3boot_payload
cargo xwin build --xwin-cache-dir .xwin --target x86_64-pc-windows-msvc -p jc3boot_injector
wine ./target/x86_64-pc-windows-msvc/debug/jc3boot_injector.exe "$@"
