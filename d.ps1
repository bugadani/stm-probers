<# #>
param (
    [Parameter(Mandatory=$true)]
    [string]$CMD,

    [string]$peri
)

# array of (data source, repository, revision)
$DATA_SOURCES = @(
    @{ # TODO: remove this
        name = "stm32-data-sources";
        repo = "https://github.com/embassy-rs/stm32-data-sources.git";
        rev = "1a3751550575f8ffee5e45307713a3e08bc9ffb4"
    },
    @{
        name = "stm32-data-generated";
        repo = "https://github.com/embassy-rs/stm32-data-generated.git";
        rev = "182f1188a45366feb2a3ba35df8317fc680c8372"
    },
    @{
        name = "probe-rs";
        repo = "https://github.com/probe-rs/probe-rs.git";
        rev = "4fd36e20d3a7eaad902e88b95b89b010843e1bd2"
    }
)

Switch ($CMD)
{
    "download-all" {
        rm -r -Force ./sources/ -ErrorAction SilentlyContinue

        # download the generated data
        foreach ($source in $DATA_SOURCES) {
            echo "Downloading $($source.name)"

            git clone $source.repo ./sources/$($source.name) -q
            pushd ./sources/$($source.name)/
            git checkout $($source.rev)
            popd
        }
    }
    "gen" {
        rm -r -Force ./output/ -ErrorAction SilentlyContinue
        cargo run --release
    }
    default {
        echo "unknown command"
    }
}
