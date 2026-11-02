$ErrorActionPreference = 'Stop'
Push-Location $PSScriptRoot
try {
    cargo fmt --check
    if ($LASTEXITCODE -ne 0) { throw 'Formatting check failed' }
    cargo clippy --all-targets --locked -- -D warnings
    if ($LASTEXITCODE -ne 0) { throw 'Clippy failed' }
    cargo test --locked
    if ($LASTEXITCODE -ne 0) { throw 'Tests failed' }
    cargo build --release --locked --target x86_64-pc-windows-msvc
    if ($LASTEXITCODE -ne 0) { throw 'Release build failed' }
    New-Item -ItemType Directory -Path dist -Force | Out-Null
    Copy-Item -LiteralPath target/x86_64-pc-windows-msvc/release/AutoWindowAdjust.exe -Destination dist/AutoWindowAdjust.exe
    Copy-Item -LiteralPath LICENSE -Destination dist/LICENSE.txt
    Copy-Item -LiteralPath THIRD-PARTY-NOTICES.txt -Destination dist/THIRD-PARTY-NOTICES.txt
    $exeHash = (Get-FileHash -LiteralPath dist/AutoWindowAdjust.exe -Algorithm SHA256).Hash.ToLowerInvariant()
    "$exeHash  AutoWindowAdjust.exe" | Set-Content -LiteralPath dist/SHA256SUMS.txt -Encoding ascii
} finally { Pop-Location }
