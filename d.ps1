<# #>
param (
    [Parameter(Mandatory=$true)]
    [string]$CMD,

    [string]$peri
)

# https://github.com/embassy-rs/stm32-data-sources
$STM_REV="75952a08c9f5491aeaa044d390f14678f15e67b9"
# https://github.com/probe-rs/probe-rs
$PROBE_RS_REV="4fd36e20d3a7eaad902e88b95b89b010843e1bd2"

Switch ($CMD)
{
    "download-all" {
        rm -r -Force ./sources/ -ErrorAction SilentlyContinue
        git clone https://github.com/embassy-rs/stm32-data-sources.git ./sources/embassy -q
        git clone https://github.com/probe-rs/probe-rs.git ./sources/probe-rs -q
        cd ./sources/embassy/
        git checkout $STM_REV
        cd ../..

        cd ./sources/probe-rs/
        git checkout $PROBE_RS_REV
        cd ../..
    }
    "gen" {
        rm -r -Force ./output/ -ErrorAction SilentlyContinue
        cargo run --release
    }
    default {
        echo "unknown command"
    }
}
