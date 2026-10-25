// Calls into jarsnitch.wasm. Shared by worker.js and scripts/check-web.mjs.
//
// The module exports its memory and five functions: input(slot, len) returns where to
// write `len` bytes for an input slot (0 old name, 1 old bytes, 2 new name, 3 new bytes;
// scan uses 0 and 1), scan() and diff() return the length of the JSON result,
// output_ptr() says where it is, and clear() frees everything.

const encoder = new TextEncoder();
const decoder = new TextDecoder();

export class Engine {
  /** @param {WebAssembly.Module} module */
  constructor(module) {
    this.wasm = new WebAssembly.Instance(module, {}).exports;
  }

  scan(name, bytes) {
    this.#put(0, encoder.encode(name));
    this.#put(1, bytes);
    return this.#result(this.wasm.scan());
  }

  diff(oldName, oldBytes, newName, newBytes) {
    this.#put(0, encoder.encode(oldName));
    this.#put(1, oldBytes);
    this.#put(2, encoder.encode(newName));
    this.#put(3, newBytes);
    return this.#result(this.wasm.diff());
  }

  #put(slot, bytes) {
    const ptr = this.wasm.input(slot, bytes.length);
    // input() may have grown the memory, so the view is made after it returns.
    new Uint8Array(this.wasm.memory.buffer, ptr, bytes.length).set(bytes);
  }

  #result(len) {
    const text = decoder.decode(new Uint8Array(this.wasm.memory.buffer, this.wasm.output_ptr(), len));
    this.wasm.clear();
    if (!text) throw new Error("the analyzer returned no result");
    return JSON.parse(text);
  }
}
