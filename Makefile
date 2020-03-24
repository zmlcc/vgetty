all: musl

musl: clean
	cargo build --release --target=x86_64-unknown-linux-musl

clean:
	cargo clean
