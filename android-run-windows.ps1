$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $false

$AndroidDir = if ($env:ANDROID_DIR) { $env:ANDROID_DIR } else { "/data/local/tmp" }
$LocalBinary = Join-Path $PSScriptRoot "nl2sh"
$RemoteBinary = "$AndroidDir/nl2sh"
$AdbArgs = @()

if (-not (Get-Command adb -ErrorAction SilentlyContinue)) {
    throw "adb was not found in PATH"
}
if (-not (Test-Path -LiteralPath $LocalBinary -PathType Leaf)) {
    throw "nl2sh was not found next to this script: $LocalBinary"
}
if ($AndroidDir -notmatch '^/[A-Za-z0-9._/-]+$') {
    throw "ANDROID_DIR must be a safe absolute Android path: $AndroidDir"
}
if ($env:ADB_SERIAL) {
    $AdbArgs += @("-s", $env:ADB_SERIAL)
}

$DeviceState = (& adb @AdbArgs get-state 2>$null | Out-String).Trim()
if ($LASTEXITCODE -ne 0 -or $DeviceState -ne "device") {
    throw "no usable adb device; connect one or set ADB_SERIAL"
}

Write-Host "Restarting adbd with root privileges..."
& adb @AdbArgs root
& adb @AdbArgs wait-for-device
if ($LASTEXITCODE -ne 0) {
    throw "adb device did not reconnect after adb root"
}

$DeviceUid = (& adb @AdbArgs shell id -u 2>$null | Out-String).Trim()
$AdbIsRoot = $LASTEXITCODE -eq 0 -and $DeviceUid -eq "0"
if ($AdbIsRoot) {
    Write-Host "adbd is running as root."
} else {
    Write-Warning "adb root is unsupported or was denied; adbd remains non-root."
}

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
    if ($LASTEXITCODE -ne 0) {
        throw "adb root and su are unavailable, and $RemoteConfig is not readable; permissions were left unchanged to protect the API key"
    }
}

Write-Warning "adb root and su are unavailable; starting as adb shell user."
Write-Host "Press Ctrl+Q in nl2sh to exit."
& adb @AdbArgs shell -t $RemoteBinary
exit $LASTEXITCODE
