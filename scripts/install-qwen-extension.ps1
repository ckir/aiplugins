#Requires -Version 5.1
<#
.SYNOPSIS
    Install a Qwen Code extension from a GitHub release zip.

.DESCRIPTION
    Works around the silent-failure bug in `qwen extensions install` on Qwen Code
    v0.22.x (https://github.com/QwenLM/qwen-code/issues/10741) by downloading
    and extracting the bundle directly into ~/.qwen/extensions/<name>/.

.PARAMETER ExtensionName
    The extension to install: re-ghidra-mcp-qwen or rtk-mcp-qwen

.EXAMPLE
    # One-liner install (PowerShell):
    irm https://raw.githubusercontent.com/ckir/aiplugins/main/scripts/install-qwen-extension.ps1 | iex; Install-QwenExtension re-ghidra-mcp-qwen

    # Or run directly:
    .\scripts\install-qwen-extension.ps1 -ExtensionName re-ghidra-mcp-qwen
#>
[CmdletBinding()]
param(
    [Parameter(Position = 0)]
    [ValidateSet('re-ghidra-mcp-qwen', 'rtk-mcp-qwen')]
    [string]$ExtensionName
)

$ErrorActionPreference = 'Stop'

function Install-QwenExtension {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory, Position = 0)]
        [ValidateSet('re-ghidra-mcp-qwen', 'rtk-mcp-qwen')]
        [string]$Name
    )

    $repo = 'ckir/aiplugins'
    $zipName = "$Name-extension.zip"
    $url = "https://github.com/$repo/releases/latest/download/$zipName"
    $extDir = Join-Path $env:USERPROFILE ".qwen\extensions\$Name"

    Write-Host "Installing Qwen Code extension: $Name" -ForegroundColor Cyan
    Write-Host "  from: $url"
    Write-Host "  to:   $extDir"
    Write-Host ""

    # Download to temp
    $tmpZip = Join-Path $env:TEMP $zipName
    Write-Host "Downloading..."
    try {
        $ProgressPreference = 'SilentlyContinue'  # speeds up Invoke-WebRequest
        Invoke-WebRequest -Uri $url -OutFile $tmpZip -UseBasicParsing -MaximumRedirection 5
    }
    catch {
        Write-Error "Download failed: $_"
        return
    }

    # Validate
    try {
        $zip = [System.IO.Compression.ZipFile]::OpenRead($tmpZip)
        $hasManifest = $zip.Entries | Where-Object { $_.FullName -eq 'qwen-extension.json' }
        $zip.Dispose()
        if (-not $hasManifest) {
            Write-Error "$zipName does not contain qwen-extension.json — not a valid Qwen extension."
            return
        }
    }
    catch {
        Write-Error "Failed to inspect zip: $_"
        return
    }

    # Back up existing
    if (Test-Path $extDir) {
        $backup = "$extDir.bak.$(Get-Date -Format 'yyyyMMddHHmmss')"
        Write-Host "Backing up existing installation to $(Split-Path $backup -Leaf)"
        Rename-Item -Path $extDir -NewName (Split-Path $backup -Leaf)
    }

    # Extract
    New-Item -ItemType Directory -Force -Path $extDir | Out-Null
    Expand-Archive -Path $tmpZip -DestinationPath $extDir -Force

    # Clean up temp
    Remove-Item $tmpZip -Force -ErrorAction SilentlyContinue

    Write-Host ""
    Write-Host "Extension '$Name' installed to $extDir" -ForegroundColor Green
    Write-Host ""

    # Verify
    if (Get-Command qwen -ErrorAction SilentlyContinue) {
        Write-Host "Verifying with 'qwen extensions list'..."
        Write-Host ""
        $list = qwen extensions list 2>&1
        $match = $list | Select-String $Name
        if ($match) {
            $match | ForEach-Object { Write-Host $_.Line }
        }
        else {
            Write-Host "(extension should appear on next 'qwen extensions list')"
        }
    }

    Write-Host ""
    Write-Host "Done. Restart any open Qwen Code session to pick up the extension." -ForegroundColor Cyan
}

# When invoked directly with a parameter
if ($ExtensionName) {
    Install-QwenExtension -Name $ExtensionName
}
else {
    Write-Host "Usage: .\install-qwen-extension.ps1 -ExtensionName <name>" -ForegroundColor Yellow
    Write-Host ""
    Write-Host "Available extensions:" -ForegroundColor Yellow
    Write-Host "  re-ghidra-mcp-qwen   Ghidra MCP for Qwen Code (19 RE tools)"
    Write-Host "  rtk-mcp-qwen         RTK command rewriter hook"
    Write-Host ""
    Write-Host "Or use the function directly after piping:" -ForegroundColor Yellow
    Write-Host '  Install-QwenExtension re-ghidra-mcp-qwen'
}
