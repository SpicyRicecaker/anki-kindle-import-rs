file path is at `$HOME/Calibre Library/Kindle/My Clippings (13)/My Clippings - Kindle.txt`

## example usage

```shell
cargo run --release -- --start-date 03-14-2022
```

Will turn the kindle clippings from that date onwards into a format that is easily readable, and write it to the `output.md` file. Add definitions and extra content to the cards in `output.md`. Turn it into a `output.json` format readable by `anki-kindle-import-py`.

```shell
cargo run --release -- --validate
```

This file can be fed into the app directly.