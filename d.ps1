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
        rev = "3dfb70953b19579eebd28407847084a89d4d9949"
    },
    @{
        name = "probe-rs";
        repo = "https://github.com/probe-rs/probe-rs.git";
        rev = "69ff0b5ee70eb70e0fe04681c2fea4e4b270005a"
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
