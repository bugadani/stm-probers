<# #>
param (
    [Parameter(Mandatory=$true)]
    [string]$CMD,

    [string]$peri
)

# array of (data source, repository, revision)
$DATA_SOURCES = @(
    @{
        name = "stm32-data-generated";
        repo = "https://github.com/embassy-rs/stm32-data-generated.git";
        rev = "ede7414a7e673ece368bd697ff72de72284985d9"
    },
    @{
        name = "probe-rs";
        repo = "https://github.com/probe-rs/probe-rs.git";
        rev = "2e72e1c0bffa78153994f921abc728d864b8201e"
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
