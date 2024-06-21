# Define colors
$colors = @{
    "WHITE" = "`e[1;97m"
    "GREEN" = "`e[1;92m"
    "RED" = "`e[1;91m"
    "YELLOW" = "`e[1;93m"
    "BLUE" = "`e[1;94m"
    "OFF" = "`e[0m"
    "TITLE" = "==============================================================>"
}

# Logging functions
function title($msg) {
    Write-Host "`n$($colors.BLUE)$($colors.TITLE)$($colors.WHITE) $msg$($colors.OFF)"
}

function fail($msg) {
    Write-Host "$($colors.RED)$($colors.TITLE)$($colors.WHITE) $msg$($colors.OFF)"
    exit 1
}

function ok($msg) {
    Write-Host "$($colors.GREEN)$($colors.TITLE)$($colors.WHITE) $msg$($colors.OFF)"
}

function finish() {
    Write-Host "`n`n`n$($colors.GREEN)$($colors.TITLE)$($colors.WHITE) MECOMP Build OK.$($colors.OFF)"
}

# Help message
function help() {
    Write-Host "./x.ps1 [ARG]"
    Write-Host ""
    Write-Host "Lint/test/build all packages in the MECOMP repo."
    Write-Host "Builds are done with --release mode."
    Write-Host ""
    Write-Host "Arguments:"
    Write-Host "    c | clippy    lint all packages"
    Write-Host "    t | test      test all packages"
    Write-Host "    b | build     build all packages"
}

# Build function
function build() {
    $components = @('mecomp-cli', 'mecomp-tui', 'mecomp-daemon')
    foreach ($component in $components) {
        title "Build [$component]"
        if (cargo build -r -p $component) {
            ok "Build [$component] OK"
        }
        else {
            fail "Build [$component] FAIL"
        }
    }

    finish
    Get-ChildItem -Path target/release/mecomp-daemon | Format-List -Property FullName
    Get-ChildItem -Path target/release/mecomp-cli | Format-List -Property FullName
    Get-ChildItem -Path target/release/mecomp-tui | Format-List -Property FullName
}

# Clippy function (lint)
function clippy() {
    # Implement clippy linting here
}

# Test function
function test() {
    # Implement testing here
}

# Do everything function
function all() {
    clippy
    test
    build
}

# Subcommands handling
switch ($args[0]) {
    'a' { all }
    'all' { all }
    'c' { clippy }
    'clippy' { clippy }
    't' { test }
    'test' { test }
    'b' { build }
    'build' { build }
    default { help }
}