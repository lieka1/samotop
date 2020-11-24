#!/bin/bash

# cleanups for CI caching

du -shx target
find target -name 'samotop*' -not -name samotop-server -exec rm -rf {} \;
find target -name 'libsamotop*' -not -name samotop-server -exec rm -rf {} \;
rm -rf target/debug/examples
rm -rf target/debug/incremental
rm -rf target/release/examples
rm -rf target/release/incremental
cargo sweep -t 3
cargo sweep -i
du -shx target