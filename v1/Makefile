# Cyclone Rescue — one-liners for the common flows.
# `make` (or `make run`) builds and launches the game with sound.

CARGO ?= cargo
BIN    = target/release/cyclone-rescue

.PHONY: all run run-silent build release test lint fmt sfx setup clean

all: run

## Launch (debug). On Debian/Ubuntu boxes without libasound2-dev this is
## still fully wired for sound thanks to .cargo/config.toml + the symlink
## created by `setup` below — no sudo needed.
run: setup
	$(CARGO) run

## Silent build/run (skips the audio backend entirely).
run-silent:
	$(CARGO) run --no-default-features

build:
	$(CARGO) build

release:
	$(CARGO) build --release
	@ls -lh $(BIN)

test:
	$(CARGO) test

lint:
	$(CARGO) clippy --all-targets -- -D warnings
	$(CARGO) fmt --check

fmt:
	$(CARGO) fmt

## Regenerate the synthesised SFX WAVs in assets/ (all original material).
sfx:
	rustc -O tools/gen_sfx.rs -o target/gen_sfx
	./target/gen_sfx

## Make `-lasound` resolvable without the *_-dev package: symlink the
## runtime lib under vendor/alsa where .cargo/config.toml looks.
## Nothing happens if ALSA is already findable or this isn't Linux.
setup:
	@test -f vendor/alsa/libasound.so && echo "vendor/alsa: present" || \
	for p in /usr/lib/*/libasound.so.2 /usr/lib/libasound.so.2 /usr/local/lib/libasound.so.2; do \
	  [ -f "$$p" ] || continue; \
	  mkdir -p vendor/alsa; \
	  ln -s "$$p" vendor/alsa/libasound.so; \
	  echo "vendor/alsa: linked -> $$p"; \
	  break; \
	done; true

clean:
	$(CARGO) clean
	rm -rf vendor
