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
        rev = "cd7eb44ba279f818c421c12724d00bf0025aa293"
    },
    @{
        name = "probe-rs";
        repo = "https://github.com/probe-rs/probe-rs.git";
        rev = "16043b0d75b6249db3038fbaeb953a91687ae3d4"
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
