# install.ps1 — one-command installer for hum (Windows)
#
# Usage:
#   irm https://raw.githubusercontent.com/Devendra116/hum/main/install.ps1 | iex
#
# Pin a version:
#   $env:HUM_VERSION = "v0.1.0"; irm https://raw.githubusercontent.com/Devendra116/hum/main/install.ps1 | iex
#
# Security:
#   - All downloads over HTTPS from official GitHub repos only
#   - SHA-256 checksum verification for hum and yt-dlp binaries
#   - Installs to user directory by default — no admin required for binaries
#   - Temp files cleaned up after install

$ErrorActionPreference = "Stop"

$Repo       = "Devendra116/hum"
$Binary     = "hum"
$InstallDir = if ($env:HUM_INSTALL_DIR) { $env:HUM_INSTALL_DIR } else { "$env:USERPROFILE\.local\bin" }
$YtdlpRepo  = "yt-dlp/yt-dlp"

function Info($msg)  { Write-Host "[info]  $msg" -ForegroundColor Cyan }
function Ok($msg)    { Write-Host "[ ok ]  $msg" -ForegroundColor Green }
function Warn($msg)  { Write-Host "[warn]  $msg" -ForegroundColor Yellow }
function Fail($msg)  { Write-Host "[error] $msg" -ForegroundColor Red; exit 1 }

Fail "Windows is not officially supported yet. Current releases only ship Linux/macOS assets. Track progress in the repository issues."

function Verify-Checksum($file, $expected, $label) {
    $actual = (Get-FileHash -Path $file -Algorithm SHA256).Hash.ToLower()
    if ($actual -ne $expected) {
        Fail "Checksum mismatch for ${label}!`n  expected: $expected`n  got:      $actual`nThe download may be corrupted or tampered with. Aborting."
    }
    Ok "Checksum verified for $label"
}

# ── detect architecture ──────────────────────────────────────────────────────

function Get-Target {
    $arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
    switch ($arch) {
        "X64"   { return "x86_64-pc-windows-msvc" }
        "Arm64" { return "aarch64-pc-windows-msvc" }
        default { Fail "Unsupported architecture: $arch" }
    }
}

# ── resolve version ──────────────────────────────────────────────────────────

function Get-Version {
    if ($env:HUM_VERSION) {
        Info "Using pinned version: $env:HUM_VERSION"
        return $env:HUM_VERSION
    }

    Info "Fetching latest release..."
    $release = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest" -Headers @{ "User-Agent" = "hum-installer" }
    $version = $release.tag_name
    if (-not $version) { Fail "Could not determine latest version. Set `$env:HUM_VERSION manually." }
    Info "Latest version: $version"
    return $version
}

# ── check existing install ───────────────────────────────────────────────────

function Check-Existing($version) {
    $existing = Get-Command $Binary -ErrorAction SilentlyContinue
    if ($existing) {
        $current = & $Binary --version 2>&1 | Select-Object -First 1
        Info "Found existing install: $current"

        $clean = $version -replace '^v', ''
        if ($current -match [regex]::Escape($clean)) {
            Info "Already up to date ($version). Nothing to do."
            exit 0
        }
        Info "Upgrading to $version..."
    } else {
        Info "No existing install found. Installing $version..."
    }
}

# ── download & install hum ───────────────────────────────────────────────────

function Download-And-Install($version, $target) {
    $asset     = "$Binary-$version-$target.zip"
    $checksums = "$Binary-$version-checksums.txt"
    $baseUrl   = "https://github.com/$Repo/releases/download/$version"

    $tmpDir = Join-Path ([System.IO.Path]::GetTempPath()) "hum-install-$(Get-Random)"
    New-Item -ItemType Directory -Path $tmpDir -Force | Out-Null

    try {
        Info "Downloading $asset..."
        Invoke-WebRequest -Uri "$baseUrl/$asset" -OutFile "$tmpDir\$asset" -UseBasicParsing

        Info "Downloading checksums..."
        try {
            Invoke-WebRequest -Uri "$baseUrl/$checksums" -OutFile "$tmpDir\$checksums" -UseBasicParsing
            $lines = Get-Content "$tmpDir\$checksums" | Where-Object { $_ -match [regex]::Escape($asset) }
            if ($lines) {
                $expected = ($lines -split '\s+')[0]
                Verify-Checksum "$tmpDir\$asset" $expected "hum"
            } else {
                Warn "Asset not found in checksums file - skipping verification"
            }
        } catch {
            Warn "Checksums file not available - skipping verification"
        }

        Info "Extracting..."
        Expand-Archive -Path "$tmpDir\$asset" -DestinationPath "$tmpDir\extracted" -Force

        if (-not (Test-Path $InstallDir)) {
            New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
        }

        $bin = Get-ChildItem -Path "$tmpDir\extracted" -Recurse -Filter "$Binary.exe" | Select-Object -First 1
        if (-not $bin) { Fail "Could not find '$Binary.exe' in the archive" }

        Copy-Item -Path $bin.FullName -Destination "$InstallDir\$Binary.exe" -Force
        Ok "Installed hum to $InstallDir\$Binary.exe"
    } finally {
        Remove-Item -Path $tmpDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}

# ── install yt-dlp (standalone exe, no Python) ──────────────────────────────

function Install-Ytdlp {
    if (Get-Command "yt-dlp" -ErrorAction SilentlyContinue) {
        Ok "yt-dlp already installed"
        return
    }

    Info "Installing yt-dlp (standalone binary - no Python required)..."

    $ytdlpAsset = "yt-dlp.exe"
    $ytdlpUrl   = "https://github.com/$YtdlpRepo/releases/latest/download/$ytdlpAsset"
    $ytdlpSums  = "https://github.com/$YtdlpRepo/releases/latest/download/SHA2-256SUMS"

    $tmpDir = Join-Path ([System.IO.Path]::GetTempPath()) "ytdlp-install-$(Get-Random)"
    New-Item -ItemType Directory -Path $tmpDir -Force | Out-Null

    try {
        Info "Downloading yt-dlp.exe..."
        try {
            Invoke-WebRequest -Uri $ytdlpUrl -OutFile "$tmpDir\$ytdlpAsset" -UseBasicParsing
        } catch {
            Warn "Failed to download yt-dlp. You'll need to install it manually."
            return
        }

        Info "Verifying yt-dlp checksum..."
        try {
            Invoke-WebRequest -Uri $ytdlpSums -OutFile "$tmpDir\SHA2-256SUMS" -UseBasicParsing
            $lines = Get-Content "$tmpDir\SHA2-256SUMS" | Where-Object { $_ -match "yt-dlp\.exe$" }
            if ($lines) {
                $expected = ($lines -split '\s+')[0]
                Verify-Checksum "$tmpDir\$ytdlpAsset" $expected "yt-dlp"
            } else {
                Warn "yt-dlp asset not found in checksums - skipping verification"
            }
        } catch {
            Warn "yt-dlp checksums not available - skipping verification"
        }

        if (-not (Test-Path $InstallDir)) {
            New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
        }

        Copy-Item -Path "$tmpDir\$ytdlpAsset" -Destination "$InstallDir\yt-dlp.exe" -Force
        Ok "Installed yt-dlp to $InstallDir\yt-dlp.exe"
    } finally {
        Remove-Item -Path $tmpDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}

# ── install mpv ──────────────────────────────────────────────────────────────

function Install-Mpv {
    if (Get-Command "mpv" -ErrorAction SilentlyContinue) {
        Ok "mpv already installed"
        return
    }

    Info "mpv is required for audio playback. Attempting to install..."

    if (Get-Command "winget" -ErrorAction SilentlyContinue) {
        Info "Installing mpv via winget..."
        try {
            winget install --id=mpv-player.mpv --accept-package-agreements --accept-source-agreements
            Ok "mpv installed via winget"
            return
        } catch { }
    }

    if (Get-Command "scoop" -ErrorAction SilentlyContinue) {
        Info "Installing mpv via scoop..."
        try {
            scoop install mpv
            Ok "mpv installed via scoop"
            return
        } catch { }
    }

    if (Get-Command "choco" -ErrorAction SilentlyContinue) {
        Info "Installing mpv via Chocolatey..."
        try {
            choco install mpv -y
            Ok "mpv installed via Chocolatey"
            return
        } catch { }
    }

    Warn "Could not auto-install mpv"
    Write-Host ""
    Write-Host "  Please install mpv manually:"
    Write-Host "    winget install mpv"
    Write-Host "    scoop install mpv"
    Write-Host "    https://mpv.io/installation/"
    Write-Host ""
}

# ── PATH check ───────────────────────────────────────────────────────────────

function Check-Path {
    $paths = $env:PATH -split ';'
    $normalized = $paths | ForEach-Object { $_.TrimEnd('\', '/') }
    $target = $InstallDir.TrimEnd('\', '/')

    if ($normalized -notcontains $target) {
        Warn "$InstallDir is not in your PATH"
        Write-Host ""
        Write-Host "  Add it permanently:"
        Write-Host "    [Environment]::SetEnvironmentVariable('PATH', `$env:PATH + ';$InstallDir', 'User')"
        Write-Host ""
        Write-Host "  Then restart your terminal."
        Write-Host ""
    }
}

# ── final status ─────────────────────────────────────────────────────────────

function Print-Status {
    Write-Host ""
    Write-Host "  ────────────────────────────────────"
    Write-Host "  Status:"
    Write-Host ""

    $humOk    = (Get-Command "hum"    -ErrorAction SilentlyContinue) -or (Test-Path "$InstallDir\hum.exe")
    $ytdlpOk  = (Get-Command "yt-dlp" -ErrorAction SilentlyContinue) -or (Test-Path "$InstallDir\yt-dlp.exe")
    $mpvOk    = [bool](Get-Command "mpv" -ErrorAction SilentlyContinue)

    if ($humOk)   { Ok "hum     installed" } else { Warn "hum     not in PATH (installed to $InstallDir\hum.exe)" }
    if ($ytdlpOk) { Ok "yt-dlp  installed" } else { Warn "yt-dlp  missing" }
    if ($mpvOk)   { Ok "mpv     installed" } else { Warn "mpv     missing - audio won't work until installed" }

    Write-Host "  ────────────────────────────────────"
    Write-Host ""

    if ($humOk -and $ytdlpOk -and $mpvOk) {
        Ok "All good! Run 'hum' to start playing music."
    } else {
        Info "hum is installed but some dependencies need attention (see above)."
    }
    Write-Host ""
}

# ── main ─────────────────────────────────────────────────────────────────────

Write-Host ""
Write-Host "  ♪ hum installer (Windows)"
Write-Host ""

$target  = Get-Target
Info "Detected platform: $target"

$version = Get-Version
Check-Existing $version
Download-And-Install $version $target
Install-Ytdlp
Install-Mpv
Check-Path
Print-Status
