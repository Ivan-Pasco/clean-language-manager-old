# Clean Language Manager Installer for Windows
# This script downloads and installs the latest version of cleen

param(
    [string]$InstallDir = "$env:LOCALAPPDATA\cleen\bin",
    [string]$Repo = "Ivan-Pasco/clean-language-manager-old"
)

# Configuration
$BinaryName = "cleen.exe"
$ArchiveName = "cleen-windows-x86_64.zip"

Write-Host "Clean Language Manager Installer" -ForegroundColor Blue
Write-Host "=================================" -ForegroundColor Blue
Write-Host ""

# Function to get latest release version
function Get-LatestVersion {
    try {
        $response = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest"
        return $response.tag_name
    }
    catch {
        Write-Error "Failed to fetch latest version: $_"
        exit 1
    }
}

# Function to add directory to PATH
function Add-ToPath {
    param([string]$Directory)
    
    try {
        $currentPath = [Environment]::GetEnvironmentVariable("PATH", "User")
        
        # Check if already in PATH
        if ($currentPath -like "*$Directory*") {
            Write-Host "⚠️  PATH already configured" -ForegroundColor Yellow
            return $true
        }
        
        Write-Host "Adding $Directory to PATH..." -ForegroundColor Yellow
        
        # Add to user PATH
        $newPath = if ($currentPath) { "$currentPath;$Directory" } else { $Directory }
        [Environment]::SetEnvironmentVariable("PATH", $newPath, "User")
        
        # Update current session PATH
        $env:PATH = "$env:PATH;$Directory"
        
        Write-Host "✅ PATH updated successfully!" -ForegroundColor Green
        return $true
    }
    catch {
        Write-Host "❌ Failed to update PATH: $_" -ForegroundColor Red
        return $false
    }
}

# Function to download and extract
function Install-Cleen {
    $version = Get-LatestVersion
    $downloadUrl = "https://github.com/$Repo/releases/download/$version/$ArchiveName"
    
    Write-Host "Platform: Windows x86_64" -ForegroundColor Green
    Write-Host "Version:  $version" -ForegroundColor Green
    Write-Host "Install:  $InstallDir" -ForegroundColor Green
    Write-Host ""
    
    # Create temporary directory
    $tempDir = New-TemporaryFile | ForEach-Object { Remove-Item $_; New-Item -ItemType Directory -Path $_ }
    
    try {
        Write-Host "Downloading cleen..." -ForegroundColor Yellow
        Write-Host "URL: $downloadUrl"
        
        $archivePath = Join-Path $tempDir.FullName $ArchiveName
        Invoke-WebRequest -Uri $downloadUrl -OutFile $archivePath
        
        Write-Host "Extracting archive..." -ForegroundColor Yellow
        Expand-Archive -Path $archivePath -DestinationPath $tempDir.FullName -Force
        
        # Create install directory
        if (!(Test-Path $InstallDir)) {
            New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
        }
        
        # Move binary to install directory
        $binaryPath = Join-Path $tempDir.FullName $BinaryName
        if (!(Test-Path $binaryPath)) {
            Write-Error "Binary not found in archive"
            exit 1
        }
        
        Write-Host "Installing to $InstallDir..." -ForegroundColor Yellow
        Copy-Item $binaryPath -Destination $InstallDir -Force
        
        Write-Host "✅ Clean Language Manager installed successfully!" -ForegroundColor Green
        Write-Host ""
        
        # Check and configure PATH
        $currentPath = [Environment]::GetEnvironmentVariable("PATH", "User")
        if ($currentPath -notlike "*$InstallDir*") {
            Write-Host "⚠️  Setting up PATH configuration..." -ForegroundColor Yellow
            Write-Host ""
            
            if (Add-ToPath -Directory $InstallDir) {
                Write-Host ""
                Write-Host "✅ PATH configured successfully!" -ForegroundColor Green
                Write-Host ""
                Write-Host "To use cleen immediately:" -ForegroundColor Yellow
                Write-Host "  - Restart your PowerShell/Terminal" -ForegroundColor Blue
                Write-Host "  - Or run: refreshenv (if you have Chocolatey)" -ForegroundColor Blue
                Write-Host ""
                Write-Host "You can also run cleen directly:" -ForegroundColor Yellow
                Write-Host "  $InstallDir\cleen.exe --help" -ForegroundColor Blue
            }
            else {
                Write-Host "Failed to configure PATH automatically" -ForegroundColor Red
                Write-Host ""
                Write-Host "To add cleen to your PATH manually:"
                Write-Host "1. Open PowerShell as Administrator"
                Write-Host "2. Run: [Environment]::SetEnvironmentVariable('PATH', `$env:PATH + ';$InstallDir', 'User')" -ForegroundColor Blue
                Write-Host "3. Restart your terminal"
                Write-Host ""
                Write-Host "Alternatively, run cleen directly:"
                Write-Host "  $InstallDir\cleen.exe --help" -ForegroundColor Blue
            }
        }
        else {
            Write-Host "✅ Installation directory is already in your PATH" -ForegroundColor Green
            Write-Host ""
            Write-Host "You can now run:"
            Write-Host "  cleen --help" -ForegroundColor Blue
        }
        
        Write-Host ""
        Write-Host "Next steps:" -ForegroundColor Blue
        Write-Host "1. Run: " -NoNewline; Write-Host "cleen init" -ForegroundColor Blue
        Write-Host "2. Run: " -NoNewline; Write-Host "cleen doctor" -ForegroundColor Blue
        Write-Host "3. Install a Clean Language version: " -NoNewline; Write-Host "cleen install <version>" -ForegroundColor Blue
        Write-Host ""
        Write-Host "For more information: " -NoNewline; Write-Host "https://github.com/$Repo" -ForegroundColor Blue
    }
    finally {
        # Cleanup
        Remove-Item $tempDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}

# Main execution
try {
    Install-Cleen
}
catch {
    Write-Error "Installation failed: $_"
    exit 1
}