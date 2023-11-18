#/usr/bin/env sh

cargo update && \
  cargo clean && \
  cargo b -r
