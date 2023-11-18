#/usr/bin/env sh

cargo update && \
  cargo clean && \
  cargo b -r && \
  sudo systemctl stop agate && sudo cp target/release/agate /usr/local/bin/agate && sudo systemctl start agate
