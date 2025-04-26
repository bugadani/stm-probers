#!/usr/bin/env bash

set -e
cd $(dirname $0)

CMD=$1
STM_REV=182f1188a45366feb2a3ba35df8317fc680c8372
PROBE_RS_REV=3ddfd10870619fd104e7ed5999e2c5222749405d
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
