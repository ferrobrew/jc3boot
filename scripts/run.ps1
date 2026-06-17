# Native Windows build: compile the payload + injector and run the injector.
# The injector loads jc3boot_payload.dll from its own directory.
cargo build --target x86_64-pc-windows-msvc -p jc3boot_payload
if (-not $?) {
    Write-Error "Failed to build jc3boot_payload"
    exit 1
}

cargo build --target x86_64-pc-windows-msvc -p jc3boot_injector
if (-not $?) {
    Write-Error "Failed to build jc3boot_injector"
    exit 1
}

& "./target/x86_64-pc-windows-msvc/debug/jc3boot_injector.exe" @args
