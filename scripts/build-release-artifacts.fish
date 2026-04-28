#!/usr/bin/env fish

set -g script_dir (path dirname (status filename))
set -g failures

function run_target
    set -l name $argv[1]
    set -l script $argv[2]
    echo ""
    echo "=== $name ==="
    if bash $script
        return 0
    else
        set -g failures $failures $name
    end
end

function clear_stale_linux_binaries
    set -l release_dir (path normalize $script_dir/../target/release)
    rm -f \
        $release_dir/vertexlauncher-linuxx86-64 \
        $release_dir/vertexlauncher-linuxarm64 \
        $release_dir/vertexlauncher-linux-x86_64 \
        $release_dir/vertexlauncher-linux-arm64
end

clear_stale_linux_binaries

run_target windows-x86_64  $script_dir/build-windows-x86_64.sh
run_target windows-arm64   $script_dir/build-windows-arm64.sh
run_target linux-portables $script_dir/build-linux-portables.sh
run_target macos-arm64     $script_dir/build-macos-arm64.sh

if test (count $failures) -gt 0
    echo ""
    echo "Build matrix incomplete:" >&2
    for f in $failures
        echo "  - $f" >&2
    end
    exit 1
end

echo ""
echo "All targets built."
