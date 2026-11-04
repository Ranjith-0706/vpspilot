import { describe, expect, it } from "vitest";
import { MAX_CHUNK_BYTES, base64ToBytes, chunkInput } from "./terminalData";

describe("base64ToBytes", () => {
  it("decodes ASCII, empty input and arbitrary bytes", () => {
    expect([...base64ToBytes("aGk=")]).toEqual([104, 105]);
    expect(base64ToBytes("").length).toBe(0);
    expect([...base64ToBytes("AP8A")]).toEqual([0, 255, 0]);
  });

  it("preserves multi-byte UTF-8 so xterm can decode it", () => {
    const bytes = base64ToBytes("w6k="); // "é"
    expect([...bytes]).toEqual([0xc3, 0xa9]);
    expect(new TextDecoder().decode(bytes)).toBe("é");
  });
});

describe("chunkInput", () => {
  it("returns nothing for empty input and one chunk for small input", () => {
    expect(chunkInput("")).toEqual([]);
    expect(chunkInput("ls -la\n")).toEqual(["ls -la\n"]);
  });

  it("splits large input without losing or reordering anything", () => {
    const data = "abc".repeat(40_000); // 120,000 bytes
    const chunks = chunkInput(data);
    expect(chunks.length).toBeGreaterThan(1);
    expect(chunks.join("")).toBe(data);
    for (const c of chunks)
      expect(new TextEncoder().encode(c).length).toBeLessThanOrEqual(MAX_CHUNK_BYTES);
  });

  it("never splits a multi-byte character or surrogate pair", () => {
    const data = "😀é日".repeat(5_000);
    const chunks = chunkInput(data, 10);
    expect(chunks.join("")).toBe(data);
    for (const c of chunks) {
      expect(new TextEncoder().encode(c).length).toBeLessThanOrEqual(10);
      expect(c).not.toMatch(/[\uD800-\uDBFF]$/); // no dangling high surrogate
    }
  });

  it("still emits a character larger than the limit rather than looping forever", () => {
    expect(chunkInput("😀", 2)).toEqual(["😀"]);
  });
});
