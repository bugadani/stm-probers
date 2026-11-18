#!/usr/bin/env bash

set -e
cd $(dirname $0)

CMD=$1
STM_REV=263d4a331c5d1a795100f97129cac4afa3791ce1
PROBE_RS_REV=f42ee9beb247914611c91f180ef1b4fe5067db5d
shift

case "$CMD" in
    download-all)
        rm -rf ./sources/

        git clone https://github.com/embassy-rs/stm32-data-generated.git ./sources/stm32-data-generated -q
        cd ./sources/stm32-data-generated/
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
