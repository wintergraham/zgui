#!/usr/bin/env bash
# Private Linux test loader: includes KhronosGroup/Vulkan-Loader#1866.
# Does not install or replace system libraries. Requires git, cmake >=3.22.1,
# a C compiler and Linux WSI development headers. Pass a new build directory.
set -euo pipefail
build_root=${1:?usage: build_vulkan_loader.sh NEW_BUILD_DIRECTORY}
cmake_bin=${CMAKE:-cmake}
loader_revision=363f465abadab0a8dcfc5c85d2c691e9b0b788d6
headers_revision=74d8a6cb930c68ef617b202c3ff3c59d919e086b
mkdir "$build_root"
build_root=$(cd "$build_root" && pwd)
fetch_revision() {
    local repository=$1 revision=$2 destination=$3
    git init -q "$destination"
    git -C "$destination" remote add origin "$repository"
    git -C "$destination" fetch -q --depth 1 origin "$revision"
    git -C "$destination" checkout -q --detach FETCH_HEAD
    test "$(git -C "$destination" rev-parse HEAD)" = "$revision"
}
fetch_revision https://github.com/KhronosGroup/Vulkan-Headers.git "$headers_revision" "$build_root/headers"
fetch_revision https://github.com/KhronosGroup/Vulkan-Loader.git "$loader_revision" "$build_root/loader"
"$cmake_bin" -S "$build_root/headers" -B "$build_root/headers-build" \
    -DCMAKE_INSTALL_PREFIX="$build_root/headers-install"
"$cmake_bin" --install "$build_root/headers-build"
"$cmake_bin" -S "$build_root/loader" -B "$build_root/build" \
    -DCMAKE_BUILD_TYPE=RelWithDebInfo \
    -DCMAKE_PREFIX_PATH="$build_root/headers-install" -DBUILD_TESTS=OFF
"$cmake_bin" --build "$build_root/build" --parallel 4
sha256sum "$build_root/build/loader/libvulkan.so.1.4.345"
printf 'Private loader directory: %s/build/loader\n' "$build_root"
