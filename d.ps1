<# #>
param (
    [Parameter(Mandatory=$true)]
    [string]$CMD,

    [string]$peri
)

$STM_REV="1a3751550575f8ffee5e45307713a3e08bc9ffb4"
$PROBE_RS_REV="c3ecd813d203c9b9dd94594799d20f57b31fd5b8"

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
