// Runs the analysis off the main thread so large jars don't freeze the page.
// The wasm is fetched and compiled as soon as the worker starts, so nothing is
// fetched once the page has loaded.

import { Engine } from "./engine.js";

const ready = fetch(new URL("jarsnitch.wasm", import.meta.url)).then(async (res) => {
  if (!res.ok) throw new Error(`couldn't load jarsnitch.wasm (HTTP ${res.status})`);
  return WebAssembly.compile(await res.arrayBuffer());
});
let engine = null;

ready.then(
  () => postMessage({ type: "ready" }),
  (e) => postMessage({ type: "fatal", message: e.message || String(e) }),
);

onmessage = async ({ data }) => {
  const { id, op, files } = data;
  try {
    engine ??= new Engine(await ready);
    const start = performance.now();
    const bytes = files.map((f) => new Uint8Array(f.bytes));
    const result =
      op === "scan"
        ? engine.scan(files[0].name, bytes[0])
        : engine.diff(files[0].name, bytes[0], files[1].name, bytes[1]);
    postMessage({ type: "result", id, op, result, ms: performance.now() - start });
  } catch (e) {
    // A crash inside the wasm can leave the instance unusable; start a fresh one next time.
    engine = null;
    postMessage({ type: "error", id, message: e.message || String(e) });
  }
};
