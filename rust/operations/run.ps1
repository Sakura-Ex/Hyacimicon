$ErrorActionPreference = "Stop"

Push-Location $PSScriptRoot
try {
    cargo run -- @args
}
finally {
    Pop-Location
}
