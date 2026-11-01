#!/bin/bash
# Builds carplay/prebuilt/catplay_android (static armv7 musl) inside the catplay-builder image:
#   docker build -t catplay-builder carplay/catplay_android
#   docker run --rm -v /mnt/DATA:/mnt/DATA catplay-builder /mnt/DATA/projects/android_device_amazon_sheldon/carplay/build-daemon.sh
# catplay is pinned to the same commit the g_iphone kernel modules were imported from.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
CATPLAY_REV=bc6edae
SRC=${CATPLAY_SRC:-/mnt/DATA/los-build/catplay-src}
TARGET=armv7-unknown-linux-musleabihf
# catplay_audio forces opus2/dynamic, and libopus_sys checks musl with cfg!() in build.rs (= host): force static
export LIBOPUS_STATIC=1

[ -d "$SRC/.git" ] || git clone -q https://github.com/catplay-labs/catplay "$SRC"
git -C "$SRC" fetch -q origin "$CATPLAY_REV" 2>/dev/null || true
git -C "$SRC" checkout -q -f "$CATPLAY_REV"

for p in "$HERE"/catplay_android/patches/*.patch; do git -C "$SRC" apply "$p"; done
# Android has ip/modprobe/rmmod in /system/bin, not /sbin
grep -rl '"/sbin/' "$SRC/usb" --include=*.rs | xargs sed -i 's|"/sbin/|"/system/bin/|g'
# Name shown by the car: from $CATPLAY_NAME (daemon sets it from persist.vendor.carplay.name).
# Model/bonjour instance names stay iPhone-ish so the car still treats us as an iPhone.
N='std::env::var("CATPLAY_NAME").unwrap_or("Fire TV Stick".into())'
sed -i "s|device_name: Some(\"iPhone SIM\".into())|device_name: Some($N)|; s|i_pod_name: \"CatPlay\".into()|i_pod_name: $N|" \
    "$SRC/catplay_carplay_tx_gadget/src/carplay_client_session.rs"
sed -i "s|name: \"iPhone\".into(),|name: $N,|" "$SRC/carplay/catplay_carplay/src/carplay_tx/transmitter.rs"
[ "$(cat "$SRC/catplay_carplay_tx_gadget/src/carplay_client_session.rs" "$SRC/carplay/catplay_carplay/src/carplay_tx/transmitter.rs" | grep -o CATPLAY_NAME | wc -l)" = 3 ] || { echo "name sed failed"; exit 1; }
rm -rf "$SRC/catplay_android"
cp -r "$HERE/catplay_android" "$SRC/catplay_android"
rm -rf "$SRC/catplay_android/Dockerfile" "$SRC/catplay_android/patches"
grep -q '"catplay_android"' "$SRC/Cargo.toml" || sed -i 's|^members = \[|members = [\n    "catplay_android",|' "$SRC/Cargo.toml"

cd "$SRC"
cargo build --release --target $TARGET -p catplay_android
mkdir -p "$HERE/prebuilt"
arm-linux-musleabihf-strip -o "$HERE/prebuilt/catplay_android" target/$TARGET/release/catplay_android
file "$HERE/prebuilt/catplay_android" 2>/dev/null || ls -la "$HERE/prebuilt/catplay_android"
