#!/usr/bin/env fish

set -l script_dir (path dirname (status filename))
set -l repo_root (path resolve $script_dir/..)

set -l package vertexlauncher
set -l windows_target x86_64-pc-windows-msvc
set -l release_dir $repo_root/target/release
set -l native_binary_unix $release_dir/$package
set -l native_binary_windows $release_dir/$package.exe
set -l native_binary ""
set -l windows_binary $repo_root/target/$windows_target/release/$package.exe
set -l staged_windows_binary $release_dir/$package.exe
set -l is_windows_host 0

cd $repo_root; or exit 1

if set -q WINDIR
    set is_windows_host 1
end

echo "Building native release binary..."
if test $is_windows_host -eq 1
    cargo build --release --target $windows_target -p $package
else
    cargo build --release -p $package
end
or exit $status

mkdir -p $release_dir
or exit $status

if test $is_windows_host -eq 1
    set native_binary $windows_binary
else
    if test -f $native_binary_unix
        set native_binary $native_binary_unix
    else if test -f $native_binary_windows
        set native_binary $native_binary_windows
    else
        echo "Missing native release binary: $native_binary_unix or $native_binary_windows" >&2
        exit 1
    end
end

if test $is_windows_host -eq 0
    echo "Building Windows MSVC release binary..."
    if not cargo xwin --version >/dev/null 2>&1
        echo "Missing cargo-xwin. Install it with: cargo install --locked cargo-xwin" >&2
        exit 1
    end

    env -u CFLAGS -u CXXFLAGS -u LDFLAGS -u CC -u CXX -u AR -u RANLIB \
        cargo xwin build --release --target $windows_target -p $package
    or exit $status
end

if not test -f $windows_binary
    echo "Missing Windows MSVC release binary: $windows_binary" >&2
    exit 1
end

cp -f $windows_binary $staged_windows_binary
or exit $status

echo ""
echo "Artifacts ready:"
echo "  Native:  $native_binary"
echo "  Windows: $staged_windows_binary"
