default: (build)

export toolchain := "nightly-2025-06-13"

build-all *args: (build-linux args) (build-windows args)

build-linux *args:
    cargo build --target x86_64-unknown-linux-gnu {{args}}

[windows]
build-windows *args:
    cargo build --target x86_64-pc-windows-msvc {{args}}

[linux]
build-windows *args:
    cargo build --target x86_64-pc-windows-gnu {{args}}

# build for the current detected platform
[windows]
build *args: (build-windows args)

# run for the current detected platform
[windows]
run *args: (build-windows args)
    ./target/x86_64-pc-windows-gnu/debug/ow2-server-picker.exe

# build for the current detected platform
[linux]
build *args: (build-linux args)

# run for the current detected platform
[linux]
run *args: (build-linux args)
    ./target/x86_64-unknown-linux-gnu/debug/ow2-server-picker
    
clean:
    cargo clean
    rm -r target-package

# build final packaged versions with size reduction and less debug
package:
    rustup toolchain install $toolchain
    for target in 'x86_64-pc-windows-gnu' 'x86_64-unknown-linux-gnu'; \
    do \
        rustup component add rust-src --target $target --toolchain $toolchain; \
        rustup component add rust-std --target $target --toolchain $toolchain; \
        RUSTFLAGS="-Zlocation-detail=none -Zfmt-debug=none" \
            cargo +$toolchain build \
            --profile production \
            -Z build-std=std,panic_abort \
            -Z build-std-features=panic_immediate_abort \
            -Z build-std-features="optimize_for_size" \
            --target $target; \
        mkdir -p "target-package/$target/"; \
        for fname in target/$target/production/ow2-server-picker{,.exe}; do \
            if [ -f $fname ]; then \
                cp -t target-package/$target/ $fname; \
            fi \
        done \
    done

