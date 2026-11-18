$ErrorActionPreference = "Stop"

Push-Location $PSScriptRoot
try {
    cargo run -p hsr-operations-backend -- @args
}
finally {
    Pop-Location
}
