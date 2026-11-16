// Network-free regressions for the official-release workflow, using the real WASM diff.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { checkAll, alarming } from "../web/official.js";

export async function checkOfficial(engine) {
  const read = (name) => readFile(new URL(`../fixtures/jars/${name}.jar`, import.meta.url));
  const original = await read("diff-v1");
  const updated = await read("diff-v2");
  const hash = (bytes) => createHash("sha512").update(bytes).digest("hex");
  const version = (id, number, filename, bytes, date) => ({
    id, project_id: "P", version_number: number, date_published: date, version_type: "release",
    files: [{ filename, primary: true, url: `https://fixture.invalid/${id}.jar`, hashes: { sha512: hash(bytes) } }],
  });
  const savedFetch = globalThis.fetch;
  async function run(oldName, newName, bytes, installed = original, oldVersion = "1.0.0", newVersion = "1.1.0") {
    const current = version("old", oldVersion, oldName, installed, "2026-01-01T00:00:00Z");
    const newest = version("new", newVersion, newName, bytes, "2026-02-01T00:00:00Z");
    const calls = [];
    globalThis.fetch = async (url) => {
      const u = new URL(url);
      calls.push(u.href);
      let body;
      if (u.pathname === "/v2/version_files") body = { [hash(installed)]: current };
      else if (u.pathname === "/v2/projects") body = [{ id: "P", slug: "fixture", title: "Fixture" }];
      else if (u.pathname === "/v2/version_files/update_individual") body = { [hash(installed)]: newest };
      else if (u.pathname === "/v2/project/P/version") body = [newest, current];
      else if (u.href === "https://fixture.invalid/new.jar") return new Response(bytes);
      else throw new Error(`Unexpected request: ${url}`);
      return new Response(JSON.stringify(body), { headers: { "content-type": "application/json" } });
    };
    const [result] = await checkAll([{ file: new File([installed], oldName), report: engine.scan(oldName, installed) }], {
      diff: async (a, b) => engine.diff(a.name, new Uint8Array(await a.arrayBuffer()), b.name, new Uint8Array(await b.arrayBuffer())),
    });
    return { result, calls };
  }
  try {
    for (const [oldName, newName, expected] of [
      ["Fixture-Bukkit-1.0.0.jar", "Fixture-Paper-1.1.0.jar", false],
      ["Fixture-1.0.0-1.8.jar", "NewBrand-1.1.0-1.21.jar", false],
      ["Fixture-Bukkit-1.0.0.jar", "NewBrand-1.1.0.jar", false],
      ["Fixture-1.0.0.jar", "NewBrand-Paper-1.1.0.jar", false],
      ["OldName-1.0.0.jar", "NewName-1.1.0.jar", true],
      ["OldName-Bukkit-1.0.0-1.8.jar", "NewName-Bukkit-1.1.0-1.8.jar", true],
    ]) {
      const { result, calls } = await run(oldName, newName, updated);
      assert.deepEqual(result.errors, []);
      assert.equal(Boolean(result.update), expected, `${oldName} -> ${newName}`);
      assert.equal(calls.includes("https://fixture.invalid/new.jar"), expected, "wrong builds must not be downloaded");
    }
    for (const [name, mc, expected] of [["Fixture", "1.21.1", false], ["NewName", "1.21.1", false], ["Fixture", "1.20.1", true], ["NewName", "1.20.1", true]]) {
      const { result } = await run("Fixture-1.0.0+mc1.20.1.jar", `${name}-1.1.0+mc${mc}.jar`, updated, original, "1.0.0+mc1.20.1", `1.1.0+mc${mc}`);
      assert.deepEqual(result.errors, []);
      assert.equal(Boolean(result.update), expected, `${name} / mc${mc}`);
    }
    const { result: unreadable } = await run("Fixture-1.0.0.jar", "Fixture-1.1.0.jar", Buffer.from("not a jar"));
    assert.ok(!unreadable.update);
    assert.ok(unreadable.errors.some((e) => e.includes("readable jar")));

    // Keep executable classes readable, but give the descriptor an unsupported ZIP method.
    const partialBytes = Buffer.from(await read("sample-v2"));
    let pos = partialBytes.readUInt32LE(partialBytes.length - 6);
    let changed = false;
    while (partialBytes.readUInt32LE(pos) === 0x02014b50) {
      const length = partialBytes.readUInt16LE(pos + 28);
      if (partialBytes.subarray(pos + 46, pos + 46 + length).toString() === "plugin.yml") {
        partialBytes.writeUInt16LE(99, pos + 10);
        partialBytes.writeUInt16LE(99, partialBytes.readUInt32LE(pos + 42) + 8);
        changed = true;
      }
      pos += 46 + length + partialBytes.readUInt16LE(pos + 30) + partialBytes.readUInt16LE(pos + 32);
    }
    assert.ok(changed, "partial fixture must contain the descriptor");
    const { result: partial } = await run("Fixture-1.0.0.jar", "Fixture-1.1.0.jar", partialBytes, await read("sample-v1"));
    assert.ok(partial.update?.diff.new.partial);
    assert.ok(alarming(partial), "partial comparisons must retain observed High findings");
    assert.ok(partial.errors.some((e) => e.includes("incomplete")));
    console.log("official checks ok (12 mocked update scenarios)");
  } finally {
    globalThis.fetch = savedFetch;
  }
}
