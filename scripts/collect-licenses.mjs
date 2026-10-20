// SPDX-License-Identifier: MIT
import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
const metadata = JSON.parse(execFileSync('cargo', ['metadata', '--manifest-path', 'src-tauri/Cargo.toml', '--format-version', '1', '--filter-platform', 'x86_64-pc-windows-msvc', '--locked', '--offline'], { encoding: 'utf8', maxBuffer: 20e6 }));
const packages = metadata.packages.filter(p => p.name !== 'hushline').map(p => ({ name: p.name, version: p.version, dir: path.dirname(p.manifest_path), license: p.license }));
for (const name of ['solid-js', 'lucide-solid', '@tauri-apps/api']) { const dir = path.join('node_modules', name); const p = JSON.parse(fs.readFileSync(path.join(dir, 'package.json'), 'utf8')); packages.push({ name: p.name, version: p.version, dir, license: p.license }); }
let result = 'THIRD-PARTY LICENSES\n\nHush original source: MIT, Copyright (c) 2026 Chahatesh.\nThe following dependencies retain their own ownership and terms.\nThis bundle includes Windows build/test dependencies as well as runtime dependencies.\n\n';
for (const p of packages.sort((a, b) => a.name.localeCompare(b.name) || a.version.localeCompare(b.version))) {
  result += `\n${'='.repeat(72)}\n${p.name} ${p.version} — ${p.license || 'see upstream'}\n${'='.repeat(72)}\n`;
  const files = fs.readdirSync(p.dir, { withFileTypes: true }).filter(f => f.isFile() && /^(licen[sc]e|copying|notice)([._-]|$)/i.test(f.name));
  if (!files.length) result += 'No separate top-level license file was present in the distributed package; consult its upstream source and declared license.\n';
  for (const file of files) result += `\n--- ${file.name} ---\n${fs.readFileSync(path.join(p.dir, file.name), 'utf8')}\n`;
}
result += '\nTauri NSIS installer template, tauri-cli-v2.12.3 (MIT)\n';
result += fs.readFileSync('src-tauri/windows/TAURI-LICENSE-MIT', 'utf8');
fs.writeFileSync('THIRD_PARTY_LICENSES.txt', result);
console.log(`Collected distributed license notices for ${packages.length} packages.`);
