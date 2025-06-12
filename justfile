default: (build)

build-all *args: (build-linux args) (build-windows args)

build-linux *args:
    cargo build --target x86_64-unknown-linux-gnu {{args}}

[windows]
build-windows *args:
    cargo build --target x86_64-pc-windows-gnu {{args}}

[linux]
build-windows *args:
    cargo build --target x86_64-pc-windows-msvc {{args}}

# build for the current detected platform
[windows]
build *args: (build-windows args)

# run for the current detected platform
[windows]
run *args: (build-windows args)
    ./target/x86_64-pc-windows-gnu/debug/dropship-rs.exe

# build for the current detected platform
[linux]
build *args: (build-linux args)

# run for the current detected platform
[linux]
run *args: (build-linux args)
    ./target/x86_64-unknown-linux-gnu/debug/dropship-rs

# build final packaged versions with size reduction
package:
    rustup toolchain install nightly
    rustup component add rust-src --toolchain nightly
    for target in 'x86_64-pc-windows-gnu' 'x86_64-unknown-linux-gnu'; \
    do \
        RUSTFLAGS="-Zlocation-detail=none -Zfmt-debug=none" \
            cargo +nightly build \
            --profile production \
            -Z build-std=std,panic_abort \
            -Z build-std-features=panic_immediate_abort \
            -Z build-std-features="optimize_for_size" \
            --target $target; \
        mkdir -p "target-package/$target/"; \
        cp target/$target/production/dropship-rs* target-package/$target/; \
    done

