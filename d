#!/usr/bin/env bash

set -e
cd $(dirname $0)

CMD=$1
STM_REV=ede7414a7e673ece368bd697ff72de72284985d9
PROBE_RS_REV=2e72e1c0bffa78153994f921abc728d864b8201e
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
