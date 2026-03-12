#!/usr/bin/env fish

set -l script_dir (path dirname (status filename))
set -l repo_root (path resolve $script_dir/..)

set -l package vertexlauncher
set -l windows_target x86_64-pc-windows-gnu
set -l release_dir $repo_root/target/release
set -l native_binary $release_dir/$package
set -l windows_binary $repo_root/target/$windows_target/release/$package.exe
set -l staged_windows_binary $release_dir/$package.exe

cd $repo_root; or exit 1

echo "Building native release binary..."
cargo build --release
or exit $status

echo "Building Windows GNU release binary..."
cargo build --release --target $windows_target
or exit $status

mkdir -p $release_dir
or exit $status

if not test -f $native_binary
    echo "Missing native release binary: $native_binary" >&2
    exit 1
end

if not test -f $windows_binary
    echo "Missing Windows GNU release binary: $windows_binary" >&2
    exit 1
end

cp -f $windows_binary $staged_windows_binary
or exit $status

echo ""
echo "Artifacts ready:"
echo "  Native:  $native_binary"
echo "  Windows: $staged_windows_binary"
