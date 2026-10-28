// SPDX-License-Identifier: MIT
/** Explicit binary IPC only; snapshots and configuration exports contain no WAV. */
export function recordingBlob(value: unknown): Blob {
  let bytes: Uint8Array;
  if (value instanceof ArrayBuffer) bytes = new Uint8Array(value);
  else if (Array.isArray(value) && value.length <= 24_000_000 && value.every(byte => Number.isInteger(byte) && byte >= 0 && byte <= 255)) bytes = Uint8Array.from(value);
  else throw new Error('The saved recording could not be opened.');
  if (bytes.length < 44 || bytes.length > 24_000_000 || new TextDecoder().decode(bytes.subarray(0, 4)) !== 'RIFF' || new TextDecoder().decode(bytes.subarray(8, 12)) !== 'WAVE') throw new Error('The saved recording is invalid. Nothing was played or downloaded.');
  return new Blob([new Uint8Array(bytes).buffer], { type: 'audio/wav' });
}
