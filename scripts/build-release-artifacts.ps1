$ErrorActionPreference = "Stop"

$scriptDir = $PSScriptRoot
$repoRoot = (Resolve-Path (Join-Path $scriptDir "..")).Path

$package = "vertexlauncher"
$windowsTarget = "x86_64-pc-windows-msvc"
$linuxToolchain = "stable-x86_64-unknown-linux-gnu"
$linuxTarget = "x86_64-unknown-linux-gnu"
$releaseDir = Join-Path $repoRoot "target/release"
$windowsBinary = Join-Path $repoRoot (Join-Path "target/$windowsTarget/release" "$package.exe")
$linuxBinary = Join-Path $repoRoot (Join-Path "target/$linuxTarget/release" $package)
$stagedLinuxBinary = Join-Path $releaseDir $package
$stagedWindowsBinary = Join-Path $releaseDir "$package.exe"
$crossEnvVars = @("CFLAGS", "CXXFLAGS", "LDFLAGS", "CC", "CXX", "AR", "RANLIB")

Push-Location $repoRoot
try {
    Write-Host "Building Windows MSVC release binary..."
    if ($IsWindows) {
        & cargo build --release --target $windowsTarget -p $package
        if ($LASTEXITCODE -ne 0) {
            throw "cargo build --release --target $windowsTarget -p $package failed with exit code $LASTEXITCODE"
        }
    }
    else {
        & cargo xwin --version *> $null
        if ($LASTEXITCODE -ne 0) {
            throw "Missing cargo-xwin. Install it with: cargo install --locked cargo-xwin"
        }

        $savedEnv = @{}
        foreach ($varName in $crossEnvVars) {
            if (Test-Path "Env:$varName") {
                $savedEnv[$varName] = (Get-Item "Env:$varName").Value
                Remove-Item "Env:$varName" -ErrorAction SilentlyContinue
            }
            else {
                $savedEnv[$varName] = $null
            }
        }

        try {
            & cargo xwin build --release --target $windowsTarget -p $package
            if ($LASTEXITCODE -ne 0) {
                throw "cargo xwin build --release --target $windowsTarget -p $package failed with exit code $LASTEXITCODE"
            }
        }
        finally {
            foreach ($varName in $crossEnvVars) {
                if ($null -eq $savedEnv[$varName]) {
                    Remove-Item "Env:$varName" -ErrorAction SilentlyContinue
                }
                else {
                    Set-Item "Env:$varName" $savedEnv[$varName]
                }
            }
        }
    }

    Write-Host "Building Linux GNU release binary..."
    & cargo "+$linuxToolchain" build --release --target $linuxTarget -p $package
    if ($LASTEXITCODE -ne 0) {
        throw "cargo +$linuxToolchain build --release --target $linuxTarget -p $package failed with exit code $LASTEXITCODE"
    }

    New-Item -ItemType Directory -Force -Path $releaseDir | Out-Null

    if (-not (Test-Path -LiteralPath $windowsBinary -PathType Leaf)) {
        throw "Missing Windows release binary: $windowsBinary"
    }

    if (-not (Test-Path -LiteralPath $linuxBinary -PathType Leaf)) {
        throw "Missing Linux release binary: $linuxBinary"
    }

    Copy-Item -LiteralPath $windowsBinary -Destination $stagedWindowsBinary -Force
    Copy-Item -LiteralPath $linuxBinary -Destination $stagedLinuxBinary -Force

    Write-Host ""
    Write-Host "Artifacts ready:"
    Write-Host "  Windows: $stagedWindowsBinary"
    Write-Host "  Linux:   $stagedLinuxBinary"
}
finally {
    Pop-Location
}
