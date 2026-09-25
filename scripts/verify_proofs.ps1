$ErrorActionPreference = "Stop"
Write-Host "=========================================="
Write-Host "Verifying Lean 4 NumLang Formal Proofs..."
Write-Host "=========================================="

$proofDir = Join-Path $PSScriptRoot "..\proof"
Push-Location $proofDir

try {
    lake build
    if ($LASTEXITCODE -ne 0) {
        Write-Error "Lake build failed with exit code $LASTEXITCODE"
    }

    Write-Host "Scanning for sorry placeholders..."
    $sorries = Select-String -Path "*.lean", "NumLangProofs\*.lean" -Pattern "\bsorry\b"
    if ($sorries) {
        Write-Error "ERROR: Proofs contain incomplete 'sorry' placeholders: $sorries"
    }

    Write-Host "=========================================="
    Write-Host "All Lean 4 proofs machine-checked: SUCCESS"
    Write-Host "=========================================="
} finally {
    Pop-Location
}
