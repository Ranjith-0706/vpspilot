/** Decodes the base64 output bytes sent by the backend. */
export function base64ToBytes(b64: string): Uint8Array {
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

/** Largest input chunk (in UTF-8 bytes) sent in one call; the backend rejects anything over 64 KiB. */
export const MAX_CHUNK_BYTES = 32 * 1024;

/**
 * Splits input (e.g. a large paste) into chunks that fit the backend limit without ever
 * splitting a character in half.
 */
export function chunkInput(data: string, maxBytes: number = MAX_CHUNK_BYTES): string[] {
  const encoder = new TextEncoder();
  const chunks: string[] = [];
  let current = "";
  let currentBytes = 0;
  for (const ch of data) {
    const size = encoder.encode(ch).length;
    if (currentBytes + size > maxBytes && current !== "") {
      chunks.push(current);
      current = "";
      currentBytes = 0;
    }
    current += ch;
    currentBytes += size;
  }
  if (current !== "") chunks.push(current);
  return chunks;
}
