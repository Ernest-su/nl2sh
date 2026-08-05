$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $false

$ProjectDir = $PSScriptRoot
$Target = if ($env:RUST_TARGET) { $env:RUST_TARGET } else { "aarch64-linux-android" }
$ApiLevel = if ($env:ANDROID_API_LEVEL) { $env:ANDROID_API_LEVEL } else { "26" }
$AndroidDir = if ($env:ANDROID_DIR) { $env:ANDROID_DIR } else { "/data/local/tmp" }
$RemoteBinary = "$AndroidDir/nl2sh"
$LocalBinary = Join-Path $ProjectDir "target\$Target\release\nl2sh"
$NdkDir = if ($env:ANDROID_NDK_HOME) { $env:ANDROID_NDK_HOME } elseif ($env:ANDROID_NDK_ROOT) { $env:ANDROID_NDK_ROOT } else { $null }
$AdbArgs = @()

foreach ($Command in @("adb", "cargo", "rustup")) {
    if (-not (Get-Command $Command -ErrorAction SilentlyContinue)) { throw "$Command was not found in PATH" }
}
if ([string]::IsNullOrWhiteSpace($NdkDir)) { throw "set ANDROID_NDK_HOME or ANDROID_NDK_ROOT" }
if (-not (Test-Path -LiteralPath $NdkDir -PathType Container)) { throw "Android NDK directory does not exist: $NdkDir" }
if ($ApiLevel -notmatch '^\d+$' -or [int]$ApiLevel -lt 26) { throw "ANDROID_API_LEVEL must be an integer greater than or equal to 26: $ApiLevel" }
if ($AndroidDir -notmatch '^/[A-Za-z0-9._/-]+$') { throw "ANDROID_DIR must be a safe absolute Android path: $AndroidDir" }
if ($env:ADB_SERIAL) { $AdbArgs += @("-s", $env:ADB_SERIAL) }

switch ($Target) {
    "aarch64-linux-android" { $ClangTarget = "aarch64-linux-android"; $CargoPrefix = "AARCH64_LINUX_ANDROID"; $CcSuffix = "aarch64_linux_android" }
    "armv7-linux-androideabi" { $ClangTarget = "armv7a-linux-androideabi"; $CargoPrefix = "ARMV7_LINUX_ANDROIDEABI"; $CcSuffix = "armv7_linux_androideabi" }
    default { throw "unsupported Rust target: $Target" }
}

$Toolchain = Join-Path $NdkDir "toolchains\llvm\prebuilt\windows-x86_64"
$Clang = Join-Path $Toolchain "bin\$ClangTarget$ApiLevel-clang.cmd"
$Ar = Join-Path $Toolchain "bin\llvm-ar.exe"
if (-not (Test-Path -LiteralPath $Clang -PathType Leaf) -or -not (Test-Path -LiteralPath $Ar -PathType Leaf)) { throw "NDK LLVM tools were not found under $Toolchain" }

$InstalledTargets = @(& rustup target list --installed)
if ($LASTEXITCODE -ne 0) { throw "failed to list installed Rust targets" }
if ($InstalledTargets -notcontains $Target) { throw "Rust target is missing. Run: rustup target add $Target" }

$DeviceState = (& adb @AdbArgs get-state 2>$null | Out-String).Trim()
if ($LASTEXITCODE -ne 0 -or $DeviceState -ne "device") { throw "no usable adb device; connect one or set ADB_SERIAL" }

$AdbIsRoot = $false
Write-Host "Restarting adbd with root privileges..."
$AdbRootOutput = (& adb @AdbArgs root 2>&1 | Out-String).Trim()
if ($AdbRootOutput) { Write-Host $AdbRootOutput }
& adb @AdbArgs wait-for-device
if ($LASTEXITCODE -ne 0) { throw "adb device did not reconnect after adb root" }
$DeviceUid = (& adb @AdbArgs shell id -u 2>$null | Out-String).Trim()
if ($LASTEXITCODE -eq 0 -and $DeviceUid -eq "0") { $AdbIsRoot = $true; Write-Host "adbd is running as root." } else { Write-Warning "adb root is unsupported or was denied; adbd remains non-root." }

# cc-rs builds native dependencies separately from Cargo's final link step.
Set-Item -LiteralPath "Env:CARGO_TARGET_${CargoPrefix}_LINKER" -Value $Clang
Set-Item -LiteralPath "Env:CARGO_TARGET_${CargoPrefix}_AR" -Value $Ar
Set-Item -LiteralPath "Env:CC_${CcSuffix}" -Value $Clang
Set-Item -LiteralPath "Env:AR_${CcSuffix}" -Value $Ar

Push-Location $ProjectDir
try {
    & cargo build --release --target $Target
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed for $Target" }
} finally { Pop-Location }
if (-not (Test-Path -LiteralPath $LocalBinary -PathType Leaf)) { throw "compiled binary was not found: $LocalBinary" }

Write-Host "Creating Android directory: $AndroidDir"
& adb @AdbArgs shell mkdir -p $AndroidDir
if ($LASTEXITCODE -ne 0) { throw "failed to create Android directory: $AndroidDir" }
Write-Host "Pushing: $LocalBinary -> $RemoteBinary"
& adb @AdbArgs push $LocalBinary $RemoteBinary
if ($LASTEXITCODE -ne 0) { throw "failed to push nl2sh" }
& adb @AdbArgs shell chmod 755 $RemoteBinary
if ($LASTEXITCODE -ne 0) { throw "failed to make nl2sh executable" }

if ($AdbIsRoot) {
    Write-Host "Starting $RemoteBinary through root adbd."
    Write-Host "Press Ctrl+Q in nl2sh to exit."
    & adb @AdbArgs shell -t $RemoteBinary
    exit $LASTEXITCODE
}

Write-Host "Trying Android su as a fallback..."
& adb @AdbArgs shell su -c id *> $null
if ($LASTEXITCODE -eq 0) {
    Write-Host "su access granted; starting $RemoteBinary as root."
    Write-Host "Press Ctrl+Q in nl2sh to exit."
    & adb @AdbArgs shell -t su -c $RemoteBinary
    exit $LASTEXITCODE
}

$RemoteConfig = "$AndroidDir/config.toml"
& adb @AdbArgs shell test -e $RemoteConfig
$ConfigExists = $LASTEXITCODE -eq 0
if ($ConfigExists) {
    & adb @AdbArgs shell test -r $RemoteConfig
    if ($LASTEXITCODE -ne 0) { throw "adb root and su are unavailable, and $RemoteConfig is not readable; permissions were left unchanged to protect the API key" }
}
Write-Warning "adb root and su are unavailable; starting as adb shell user."
Write-Host "Press Ctrl+Q in nl2sh to exit."
& adb @AdbArgs shell -t $RemoteBinary
exit $LASTEXITCODE
