default: (build)

export package_toolchain := "nightly-2025-06-12"

# build for the current detected platform
build *args:
    cargo build {{args}}

# run for the current detected platform
[windows]
run *args: (build args)
    ./target/debug/ow2-server-picker.exe

# run for the current detected platform
[linux]
run *args: (build args)
    ./target/debug/ow2-server-picker

clean:
    cargo clean

# build final packaged versions with size reduction and less debug
package:
    rustup component add rust-src --toolchain $package_toolchain
    cargo +$package_toolchain build \
        --profile production \
        -Z build-std=std,panic_abort \
        -Z build-std-features=panic_immediate_abort \
        -Z build-std-features="optimize_for_size"

