$results = @()

function Test-Tool {
    param(
        [string]$Name,
        [string]$Command,
        [string[]]$Arguments,
        [string]$Severity,
        [string]$Hint
    )

    if (-not (Get-Command $Command -ErrorAction SilentlyContinue)) {
        return [pscustomobject]@{
            Tool = $Name
            Status = $Severity
            Detail = "not found on PATH. $Hint"
        }
    }

    try {
        $version = (& $Command $Arguments 2>$null | Select-Object -First 1)
        if ([string]::IsNullOrWhiteSpace($version)) { $version = 'available' }
        return [pscustomobject]@{ Tool = $Name; Status = 'PASS'; Detail = $version }
    } catch {
        return [pscustomobject]@{
            Tool = $Name
            Status = $Severity
            Detail = "found but failed to run. $Hint"
        }
    }
}

$results += Test-Tool -Name 'Node.js' -Command 'node' -Arguments @('--version') -Severity 'FAIL' -Hint 'Install Node.js 20 or later.'
$results += Test-Tool -Name 'pnpm' -Command 'pnpm' -Arguments @('--version') -Severity 'FAIL' -Hint 'Run: corepack enable'
$results += Test-Tool -Name 'Rust' -Command 'rustc' -Arguments @('--version') -Severity 'FAIL' -Hint 'Install from https://rustup.rs'
$results += Test-Tool -Name 'Cargo' -Command 'cargo' -Arguments @('--version') -Severity 'FAIL' -Hint 'Cargo ships with rustup.'
$results += Test-Tool -Name 'Git' -Command 'git' -Arguments @('--version') -Severity 'WARN' -Hint 'Install Git for release work.'
$results += Test-Tool -Name 'yt-dlp' -Command 'yt-dlp' -Arguments @('--version') -Severity 'INFO' -Hint 'Optional: the app can install a verified managed copy.'
$results += Test-Tool -Name 'FFmpeg' -Command 'ffmpeg' -Arguments @('-version') -Severity 'INFO' -Hint 'Optional: the Windows app can install a verified managed copy.'

$webViewKeys = @(
    'HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}',
    'HKCU:\SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}'
)
$webView = $webViewKeys | Where-Object { Test-Path $_ } | Select-Object -First 1
if ($webView) {
    $version = (Get-ItemProperty $webView -ErrorAction SilentlyContinue).pv
    $results += [pscustomobject]@{ Tool = 'WebView2'; Status = 'PASS'; Detail = "runtime $version" }
} else {
    $results += [pscustomobject]@{ Tool = 'WebView2'; Status = 'WARN'; Detail = 'runtime not detected' }
}

$results | Format-Table -AutoSize
$failures = @($results | Where-Object { $_.Status -eq 'FAIL' })

if ($failures.Count -gt 0) {
    Write-Host "Preflight failed: $($failures.Count) required tool(s) missing." -ForegroundColor Red
    exit 1
}

Write-Host 'Preflight passed.' -ForegroundColor Green
