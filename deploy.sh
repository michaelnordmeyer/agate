#/usr/bin/env sh

sudo systemctl stop agate && sudo cp target/release/agate /usr/local/bin/agate && sudo systemctl start agate
