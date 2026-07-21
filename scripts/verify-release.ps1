param(
    [Parameter(Mandatory = $true)]
    [string]$Manifest,

    [Parameter(Mandatory = $true)]
    [string]$File
)

$manifestPath = (Resolve-Path -LiteralPath $Manifest).Path
$filePath = (Resolve-Path -LiteralPath $File).Path
$fileName = Split-Path -Leaf $filePath
$line = Get-Content -LiteralPath $manifestPath | Where-Object { $_ -match "\*$([regex]::Escape($fileName))$" } | Select-Object -First 1

if (-not $line) {
    Write-Error "No SHA-256 entry exists for $fileName."
    exit 1
}

$expected = ($line -split '\s+')[0].ToLowerInvariant()
$actual = (Get-FileHash -LiteralPath $filePath -Algorithm SHA256).Hash.ToLowerInvariant()

if ($actual -ne $expected) {
    Write-Error "SHA-256 mismatch for $fileName."
    exit 1
}

Write-Host "SHA-256 verified: $fileName" -ForegroundColor Green
