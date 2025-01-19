#!/usr/bin/env bash

set -e
cd $(dirname $0)

CMD=$1
STM_REV=75952a08c9f5491aeaa044d390f14678f15e67b9
shift

case "$CMD" in
    download-all)
        rm -rf ./sources/
        git clone https://github.com/embassy-rs/stm32-data-sources.git ./sources/embassy -q
        git clone https://github.com/probe-rs/probe-rs.git ./sources/probe-rs -q
        cd ./sources/embassy/
        git checkout $STM_REV
    ;;
    gen)
        rm -rf output
        cargo run --release
    ;;
    *)
        echo "unknown command"
    ;;
esac
