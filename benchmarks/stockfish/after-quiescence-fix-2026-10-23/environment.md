# Recorded environment

- Date: 2026-10-23.
- CPU: AMD Ryzen 7 5800X, 8 cores / 16 logical CPUs; x86_64.
- Compiler: rustc 1.97.1 (8bab26f4f 2026-07-14), release build.
- Opponent: official Stockfish 19 Linux x86-64 universal binary, release `sf_19`.
- Binary SHA-256: `0f83d24cc46d2c66c60f16001af5444873bc112b7d028594513426894c12da19`.
- Download: https://github.com/official-stockfish/Stockfish/releases/download/sf_19/stockfish-linux-x86-64-universal.tar.gz
- Browser tests had finished before this run; no other agent-started searches were running.

Command:

```sh
target/release/examples/stockfish_match \
  --stockfish /tmp/monumla-stockfish/stockfish/stockfish-linux-x86-64-universal \
  --games 2 --movetime-ms 1000 --max-plies 160 \
  --out benchmarks/stockfish/after-quiescence-fix-2026-10-23
```
