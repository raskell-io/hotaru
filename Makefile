PORT ?= 8646
WASM := target/wasm32-unknown-unknown/release/hotaru_core.wasm
DIST := dist

.PHONY: build serve site test probe experiment coupling clean

# The simulation, compiled for the web page.
build:
	cargo build --release --target wasm32-unknown-unknown -p hotaru-core --features web
	@if command -v wasm-opt >/dev/null; then \
		wasm-opt -O3 --enable-bulk-memory --enable-nontrapping-float-to-int $(WASM) -o web/hotaru.wasm; \
	else cp $(WASM) web/hotaru.wasm; fi
	@ls -l web/hotaru.wasm | awk '{print "web/hotaru.wasm: " $$5 " bytes"}'

serve: build
	@echo "http://localhost:$(PORT)/"
	python3 -m http.server $(PORT) --directory web

# The website is the web page as it stands, in dist/.
site: build
	rm -rf $(DIST)
	cp -R web $(DIST)

test:
	cargo test

# Headless activity report, e.g. make probe ARGS="seed=3 neurons=8192".
probe:
	cargo run --release --example probe -- $(ARGS)

# Pong under each feedback condition, e.g. make experiment ARGS="seeds=16 minutes=20";
# avoidance=1 compares stimulation avoidance with its mirrored control.
experiment:
	cargo run --release --example experiment -- $(ARGS)

# How much the ball's position reaches the motor regions, per culture.
coupling:
	cargo run --release --example coupling -- $(ARGS)

clean:
	cargo clean
	rm -rf web/hotaru.wasm $(DIST)
