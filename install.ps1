# PowerShell installation script for Free Cursor Client

$Repo = "freeai-dev/free-cursor-client"

# Get latest version from GitHub
Write-Host "Fetching latest version..."
try {
    $LatestRelease = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest"
    $Version = $LatestRelease.tag_name -replace '^v'
    Write-Host "Latest version: $Version"
}
catch {
    Write-Host "Failed to fetch release information from GitHub"
    exit 1
}

$DownloadUrl = "https://github.com/$Repo/releases/download/v$Version/free-cursor-client.exe"

# Download and run the program
Write-Host "Downloading latest version..."
$TempFile = Join-Path $env:TEMP ([System.IO.Path]::GetRandomFileName() + ".exe")
try {
    Invoke-WebRequest -Uri $DownloadUrl -OutFile $TempFile
    Write-Host "Download complete, launching program..."
    & $TempFile $args
}
catch {
    Write-Host "Failed to download or run program"
    exit 1
}
finally {
    # Clean up temp file after program exits
    Remove-Item -Path $TempFile -Force -ErrorAction SilentlyContinue
}