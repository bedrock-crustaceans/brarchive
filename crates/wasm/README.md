# @bedrock-crustaceans/brarchive

Read and write **Bedrock Archive** (`.brarchive`) files from JavaScript and TypeScript.
This is the bundling format Mojang uses to pack the files inside Minecraft Bedrock Edition
resource and behavior packs.

The format is implemented in Rust ([`brarchive` on crates.io](https://crates.io/crates/brarchive))
and compiled to WebAssembly. Entries are usually JSON, but newer packs also embed compiled
binary data, and both are handled as raw bytes.

## Install

The package is published to [JSR](https://jsr.io/@bedrock-crustaceans/brarchive).

**Deno**

```shell
deno add jsr:@bedrock-crustaceans/brarchive
```

**Node, Bun, or other npm-based projects**

```shell
npx jsr add @bedrock-crustaceans/brarchive
```

Then import it:

```ts
import { deserialize, deserializeText, list, serialize } from "@bedrock-crustaceans/brarchive";
```

All functions are synchronous. The wasm module is loaded when the package is imported, so
it works in **Deno 2.1+** and in **browsers** with ES module WebAssembly integration.

## API

| Function | Returns | Description |
|---|---|---|
| `serialize(entries, options?)` | `Uint8Array` | Build an archive from name → content pairs |
| `list(bytes)` | `string[]` | Entry names, without decoding any content |
| `deserialize(bytes)` | `Map<string, Uint8Array>` | Name → raw bytes; safe for archives with binary entries |
| `deserializeText(bytes)` | `Map<string, string>` | Name → string; throws if an entry is not valid text |

`serialize` accepts a `Map`, an array of `[name, content]` tuples, or a plain object.
Values can be strings (stored as UTF-8) or `Uint8Array`s. Entries are written in the
order given. Pass `{ dedup: true }` to store identical content only once.

## Usage

```ts
import {
  deserialize,
  deserializeText,
  list,
  serialize,
} from "@bedrock-crustaceans/brarchive";

// Build an archive. Values can be strings (stored as UTF-8) or Uint8Arrays.
const bytes = serialize({
  "zombie.entity.json": '{"format_version":"1.10.0"}',
  "creeper.entity.json": new Uint8Array([0x7f, 0x4d, 0x43, 0x42]),
});

// Entry names without decoding any content
list(bytes); // ["zombie.entity.json", "creeper.entity.json"]

// Name -> bytes, safe for archives that contain binary entries
const entries = deserialize(bytes);
entries.get("creeper.entity.json"); // Uint8Array

// Name -> string, for archives that only hold text. Throws on binary entries.
const text = deserializeText(bytes);
JSON.parse(text.get("zombie.entity.json")!);

// Round-trip, storing identical content only once
serialize(entries, { dedup: true });
```

## Development

The package lives in `crates/wasm/` of the
[brarchive](https://github.com/bedrock-crustaceans/brarchive) repository. `lib/` holds the
generated bindings and is committed so publishing does not need a Rust toolchain.

| Task | What it does |
|---|---|
| `deno task wasmbuild` | Rebuild `lib/` from `src/lib.rs` |
| `deno task wasmbuild:check` | Verify `lib/` is up to date |
| `deno task test` | Run the test suite |
| `deno task publish:dry` | Dry-run a JSR publish |

The version in `deno.json` must match `[workspace.package] version` in the root
`Cargo.toml`; a test enforces this.
