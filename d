#!/usr/bin/env bash

set -e
cd $(dirname $0)

CMD=$1
STM_REV=cd7eb44ba279f818c421c12724d00bf0025aa293
PROBE_RS_REV=16043b0d75b6249db3038fbaeb953a91687ae3d4
shift

case "$CMD" in
    download-all)
        rm -rf ./sources/

        git clone https://github.com/embassy-rs/stm32-data-generated.git ./sources/stm32-data-generated -q
        cd ./sources/embassy/
        git checkout $STM_REV
        cd ../..

        git clone https://github.com/probe-rs/probe-rs.git ./sources/probe-rs -q
        cd ./sources/probe-rs/
        git checkout $PROBE_RS_REV
        cd ../..
    ;;
    gen)
        rm -rf output
        cargo run --release
    ;;
    *)
        echo "unknown command"
    ;;
esac
