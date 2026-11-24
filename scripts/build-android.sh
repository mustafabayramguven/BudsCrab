#!/usr/bin/env bash
# Build the Rust core for Android and generate the Kotlin bindings.
# Needs: rustup targets (aarch64/armv7/x86_64-linux-android), cargo-ndk,
#        ANDROID_NDK_HOME pointing at an NDK.
set -euo pipefail
cd "$(dirname "$0")/.."

case "$(uname -s)" in
  Darwin) EXT=dylib ;;
  *) EXT=so ;;
esac

echo ">> host build for binding generation"
cargo build -p budscrab-core --lib

echo ">> generating Kotlin bindings"
cargo run -q -p budscrab-core --features bindgen --bin uniffi-bindgen -- \
  generate --library "target/debug/libbudscrab_core.$EXT" \
  --language kotlin --no-format --out-dir android/app/src/main/java

# Fall back to whatever NDK the environment advertises.
if [ -z "${ANDROID_NDK_HOME:-}" ]; then
  unset ANDROID_NDK_HOME
  if [ -n "${ANDROID_NDK_LATEST_HOME:-}" ]; then export ANDROID_NDK_HOME="$ANDROID_NDK_LATEST_HOME"; fi
fi

echo ">> building Android libraries"
cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 -P 26 \
  -o android/app/src/main/jniLibs build --release -p budscrab-core --lib

echo ">> done. Now: cd android && ./gradlew assembleRelease"
