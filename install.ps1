# PowerShell installation script for Free Cursor Client

$Repo = "freeai-dev/free-cursor-client"

# Get latest version from GitHub
Write-Host "正在获取最新版本信息..."
try {
    $LatestRelease = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest"
    $Version = $LatestRelease.tag_name -replace '^v'
    Write-Host "最新版本：$Version"
}
catch {
    Write-Host "从 GitHub 获取发布信息失败"
    exit 1
}

$DownloadUrl = "https://github.com/$Repo/releases/download/v$Version/free-cursor-client.exe"

# Download and run the program
Write-Host "正在下载最新版本..."
$TempFile = Join-Path $env:TEMP ([System.IO.Path]::GetRandomFileName() + ".exe")
try {
    Invoke-WebRequest -Uri $DownloadUrl -OutFile $TempFile
    Write-Host "下载完成，正在启动程序..."
    & $TempFile $args
}
catch {
    Write-Host "下载或运行程序失败"
    exit 1
}
finally {
    # Clean up temp file after program exits
    Remove-Item -Path $TempFile -Force -ErrorAction SilentlyContinue
}