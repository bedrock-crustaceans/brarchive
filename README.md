# brarchive-rs

[![Crates.io Version](https://img.shields.io/crates/v/brarchive)](https://crates.io/crates/brarchive)
[![Crates.io Total Downloads](https://img.shields.io/crates/d/brarchive)](https://crates.io/crates/brarchive)
[![Crates.io MSRV (version)](https://img.shields.io/crates/msrv/brarchive/0.4.1)](https://crates.io/crates/brarchive)
[![Crates.io License](https://img.shields.io/crates/l/brarchive)](https://github.com/theaddonn/brarchive-rs/blob/main/LICENSE)

A library and CLI for the **Bedrock Archive** (`.brarchive`) format, the bundling format
Mojang uses to pack the files inside Minecraft Bedrock Edition resource and behavior packs.
Entries are usually JSON, but newer packs also embed compiled binary data, and brarchive-rs
handles both.

It ships in three flavours:

| | Package | Install |
|---|---|---|
| **CLI** | [`brarchive-cli`](https://crates.io/crates/brarchive-cli) on crates.io | `cargo install brarchive-cli` |
| **Rust library** | [`brarchive`](https://crates.io/crates/brarchive) on crates.io | `cargo add brarchive` |
| **JavaScript / TypeScript** | [`@bedrock-crustaceans/brarchive`](https://jsr.io/@bedrock-crustaceans/brarchive) on JSR | `deno add jsr:@bedrock-crustaceans/brarchive` |

The binary format itself is documented in [FORMAT.md](FORMAT.md).

## Installation

### CLI

Install from crates.io:

```shell
cargo install brarchive-cli
```

> **Note:** the package is called `brarchive-cli` (the plain `brarchive` name is the
> library), but the installed binary is just `brarchive`.

Prebuilt binaries for Linux, macOS, and Windows are also attached to every
[GitHub release](https://github.com/theaddonn/brarchive-rs/releases), so you can skip the
build entirely: download the one for your platform and put it somewhere on your `PATH`.

Verify it works:

```shell
brarchive --help
```

### Rust library

```shell
cargo add brarchive
```

### JavaScript / TypeScript

The same code is compiled to WebAssembly and published to JSR:

```shell
deno add jsr:@bedrock-crustaceans/brarchive
```

For Node, Bun, or other npm-based projects, use the JSR installer instead:

```shell
npx jsr add @bedrock-crustaceans/brarchive
```

See [crates/wasm/README.md](crates/wasm/README.md) for the full API.

## CLI Usage

| Subcommand | Description |
|---|---|
| `encode` | Bundle a folder into a `.brarchive` |
| `decode` | Extract a `.brarchive` into a folder |
| `list` | Print the entry names in an archive without extracting anything |

Each subcommand takes either a single `.brarchive` file or, with `--recursive`, a whole
pack whose archives live under `__brarchive/`.

### Options at a glance

| Option | Applies to | Description |
|---|---|---|
| `--recursive` | all | Operate on a whole pack: every archive under `__brarchive/` |
| `--skip-root` | `encode` | Leave files in the pack's top-level directory loose, like a vanilla pack |
| `--dedup` | `encode` | Store identical file contents only once |
| `--pretty` | `decode` | Write JSON entries with a 2-space indent instead of minified |
| `--overwrite` | `decode` | Replace files that already exist instead of stopping |
| `--delete-source` | `encode`, `decode` | Remove the originals once the archive is written / extracted |

### `encode`

Bundle a folder into one archive:

```shell
brarchive encode path/to/dir output.brarchive
```

Walk a whole pack and mirror its directory tree into `__brarchive/`, writing one archive
per folder:

```shell
brarchive encode path/to/pack --recursive
```

Files in the top-level directory itself (`manifest.json`, `pack_icon.png`, and so on) are
bundled into `__brarchive/__root__.brarchive`, and `decode --recursive` puts them back at
the root. Mojang leaves those files loose, so pass `--skip-root` if you want the same
layout as a vanilla pack:

```shell
brarchive encode path/to/pack --recursive --skip-root
```

Add `--dedup` to store identical file contents only once, and `--delete-source` to remove
the originals once the archive is written:

```shell
brarchive encode path/to/dir output.brarchive --dedup --delete-source
```

### `decode`

Extract a single archive into a folder:

```shell
brarchive decode output.brarchive path/to/out/
```

Extract every archive under a pack's `__brarchive/` folder in one go:

```shell
brarchive decode path/to/pack --recursive
```

Each archive is unpacked into the directory its path under `__brarchive/` mirrors, so
`__brarchive/textures/ui.brarchive` lands in `textures/ui/`. That directory is merged
into, not emptied, so loose files that were never archived (textures, sounds, and so on)
are left alone.

If an entry would replace a file that already exists, the decode stops before writing
anything. Pass `--overwrite` to replace those files instead, for example when
re-extracting a pack:

```shell
brarchive decode path/to/pack --recursive --overwrite
```

Mojang ships the JSON inside these archives minified, and by default the CLI writes each
entry back exactly as stored. Pass `--pretty` to get readable JSON with a 2-space indent.
Entries that are not JSON, such as the compiled binary MCB files Bedrock now embeds, are
always written untouched so nothing gets corrupted:

```shell
brarchive decode output.brarchive path/to/out/ --pretty
```

`--delete-source` works here too and removes the archive after a successful decode.

### `list`

Print the entry names in an archive without extracting anything:

```shell
brarchive list output.brarchive
```

Or list the contents of every archive in a pack:

```shell
brarchive list path/to/pack --recursive
```

## Rust Library Usage

```rust
use std::collections::BTreeMap;

// List entry names without reading any content
let names: Vec<String> = brarchive::list(&bytes)?;

// Deserialize into anything that implements FromIterator<(String, Vec<u8>)>.
// Content comes back as raw bytes, since an archive can hold binary entries.
let map: BTreeMap<String, Vec<u8>> = brarchive::deserialize(&bytes)?;
let vec: Vec<(String, Vec<u8>)>    = brarchive::deserialize(&bytes)?;

// Entries that hold text can be turned back into a String when you need one
let text = String::from_utf8(map["entity.json"].clone())?;

// Serialize any iterable of key/value pairs. Keys are entry names; values are
// bytes, so &str, String, Vec<u8>, and &[u8] all work.
brarchive::serialize([("entity.json", r#"{"id":"zombie"}"#)])?;
brarchive::serialize(&map)?;

// Serialize with options, for example deduplicating identical content
brarchive::serialize_with(&map, brarchive::SerializeOptions { dedup: true })?;
```

## JavaScript / TypeScript Usage

```ts
import { deserialize, list, serialize } from "@bedrock-crustaceans/brarchive";

const bytes = serialize({ "entity.json": '{"id":"zombie"}' });
list(bytes);                           // ["entity.json"]
deserialize(bytes).get("entity.json"); // Uint8Array
```

See [crates/wasm/README.md](crates/wasm/README.md) for the full API.

## Format

See [FORMAT.md](FORMAT.md) for the binary format specification.

## License

See [LICENSE](LICENSE).
