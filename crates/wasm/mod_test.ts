import {
  assert,
  assertEquals,
  assertGreater,
  assertLess,
  assertThrows,
} from "@std/assert";
import { deserialize, deserializeText, list, serialize } from "./mod.ts";

const fixtures = new URL("../lib/tests/fixtures/", import.meta.url);
const pack = new URL(
  "../cli/tests/fixtures/pack/__brarchive/",
  import.meta.url,
);

function readFixture(base: URL, name: string): Uint8Array {
  return Deno.readFileSync(new URL(name, base));
}

const MCB = new Uint8Array([
  0x7f,
  0x4d,
  0x43,
  0x42,
  0x01,
  0x00,
  0xd2,
  0x20,
  0xde,
  0x77,
  0xff,
  0x00,
]);

Deno.test("round trips string entries from a plain object", () => {
  const bytes = serialize({
    "b.json": '{"b":2}',
    "a.json": '{"a":1}',
  });
  const entries = deserializeText(bytes);
  assertEquals([...entries], [["b.json", '{"b":2}'], ["a.json", '{"a":1}']]);
});

Deno.test("round trips a Map with binary and text content", () => {
  const input = new Map<string, Uint8Array | string>([
    ["zombie.entity.json", '{"format_version":"1.10.0"}'],
    ["creeper.entity.json", MCB],
  ]);
  const entries = deserialize(serialize(input));
  assertEquals(
    entries.get("zombie.entity.json"),
    new TextEncoder().encode('{"format_version":"1.10.0"}'),
  );
  assertEquals(entries.get("creeper.entity.json"), MCB);
});

Deno.test("accepts an array of tuples and preserves order", () => {
  const bytes = serialize([["z", "1"], ["a", "2"], ["m", "3"]]);
  assertEquals(list(bytes), ["z", "a", "m"]);
});

Deno.test("serializes an empty archive", () => {
  const bytes = serialize({});
  assertEquals(list(bytes), []);
  assertEquals(deserialize(bytes).size, 0);
});

Deno.test("handles empty and non-ASCII content", () => {
  const entries = deserialize(
    serialize({ "empty.json": "", "ünï.json": "héllo 🌍" }),
  );
  assertEquals(entries.get("empty.json"), new Uint8Array());
  assertEquals(
    new TextDecoder().decode(entries.get("ünï.json")),
    "héllo 🌍",
  );
});

Deno.test("dedup produces a smaller archive for identical content", () => {
  const same = "x".repeat(2000);
  const input = { "a.json": same, "b.json": same, "c.json": same };
  const plain = serialize(input);
  const deduped = serialize(input, { dedup: true });
  assertLess(deduped.byteLength, plain.byteLength);
  assertEquals(deserializeText(deduped), deserializeText(plain));
});

Deno.test("deserializeText rejects binary entries", () => {
  const bytes = serialize({ "creeper.entity.json": MCB });
  assertThrows(() => deserializeText(bytes), TypeError);
  // ...while the byte-level API still works.
  assertEquals(deserialize(bytes).get("creeper.entity.json"), MCB);
});

Deno.test("rejects content that is neither string nor bytes", () => {
  assertThrows(
    // deno-lint-ignore no-explicit-any
    () => serialize({ "a.json": 42 as any }),
    TypeError,
    "must be a string or Uint8Array",
  );
});

Deno.test("rejects invalid archives", () => {
  assertThrows(() => list(new Uint8Array([1, 2, 3, 4])));
  assertThrows(() =>
    deserialize(new TextEncoder().encode("not an archive at all"))
  );
  assertThrows(() => deserialize(new Uint8Array()));
});

Deno.test("rejects an archive whose entry table runs past its end", () => {
  const bytes = serialize({ "entity.json": "{}" }).subarray(0, 17);
  assertThrows(() => list(bytes), Error, "Entry table");
  assertThrows(() => deserialize(bytes), Error, "Entry table");
});

Deno.test("reads archives produced by the Rust library", () => {
  const bytes = readFixture(fixtures, "entity.brarchive");
  const names = list(bytes);
  assertGreater(names.length, 0);
  assert(names.includes("zombie.entity.json"), names.join(", "));

  const entries = deserializeText(bytes);
  assertEquals([...entries.keys()], names);
  const zombie = JSON.parse(entries.get("zombie.entity.json")!);
  assertEquals(typeof zombie.format_version, "string");
});

Deno.test("re-encoding a fixture yields an identical archive", () => {
  const original = readFixture(fixtures, "entity.brarchive");
  const reencoded = serialize(deserialize(original));
  assertEquals(reencoded, original);
});

Deno.test("reads an empty archive fixture", () => {
  const bytes = readFixture(fixtures, "ddui.brarchive");
  assertEquals(list(bytes), []);
});

Deno.test("reads the CLI's pack fixture including its binary entry", () => {
  const entries = deserialize(readFixture(pack, "entity.brarchive"));
  assertEquals(entries.get("creeper.entity.json"), MCB);
  assertEquals(
    JSON.parse(new TextDecoder().decode(entries.get("zombie.entity.json"))),
    {
      format_version: "1.10.0",
      "minecraft:client_entity": {
        description: {
          identifier: "minecraft:zombie",
          textures: { default: "textures/entity/zombie" },
        },
      },
    },
  );
});
