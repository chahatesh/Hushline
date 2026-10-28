// SPDX-License-Identifier: MIT
import { describe, expect, it } from 'vitest';
import { recordingBlob } from './recording-data';
function fixture() { const bytes = new Uint8Array(64); bytes.set(new TextEncoder().encode('RIFF')); bytes.set(new TextEncoder().encode('WAVE'), 8); return bytes; }
describe('saved recording binary contract', () => {
  it('opens binary IPC without a base64 copy in history', async () => {
    const bytes = fixture(); const blob = recordingBlob(bytes.buffer);
    expect(blob.type).toBe('audio/wav'); expect(new Uint8Array(await blob.arrayBuffer())).toEqual(bytes);
  });
  it('accepts the JSON array fallback without changing audio', async () => {
    expect(new Uint8Array(await recordingBlob([...fixture()]).arrayBuffer())).toEqual(fixture());
  });
  it('rejects malformed IPC and non-audio payloads', () => {
    for (const value of [null, {}, 'RIFF', [256], [-1], [1.5], new ArrayBuffer(64)]) expect(() => recordingBlob(value)).toThrow();
  });
});
