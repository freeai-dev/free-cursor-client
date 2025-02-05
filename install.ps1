# PowerShell installation script for Free Cursor Client

# Get latest version from API
Write-Host "Fetching latest version..."
try {
    $Response = Invoke-RestMethod -Uri "https://auth-server.freeai.dev/api/v1/client/download"
    $Version = $Response.version
    $DownloadUrl = $Response.downloadUrl
    Write-Host "Latest version: $Version"
}
catch {
    Write-Host "Failed to fetch download information from server"
    exit 1
}

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