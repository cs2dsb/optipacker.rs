#!/usr/bin/env bash
set -eu;


EXAMPLE="${EXAMPLE:-optimize_and_pack}"
RELEASE="${RELEASE:-}"
FEATURES="${FEATURES:-}"
VERBOSE="${VERBOSE:-}"
RUST_BACKTRACE=${RUST_BACKTRACE:-}

if [[ "$RELEASE" != "" ]]; then
    RELEASE="--release"
fi

(
    cd "${BASH_SOURCE%/*}/.."

    # This is to reduce the priority of the build enough not to cause audio/video to stutter
    systemd-run \
        --user \
        --scope \
        --slice=user.slice \
        -p IOAccounting=true \
        -p CPUWeight=1 \
        -p IOWeight=1 \
                      \
        cargo \
            watch \
            -i optimized \
            -i packed \
            -i examples/packed.rs \
            -x "run $VERBOSE $RELEASE --features=\"$FEATURES\" --example $EXAMPLE"
)

