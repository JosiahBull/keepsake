#!/usr/bin/env node
import { cp, lstat, mkdir, writeFile } from 'node:fs/promises';
import { homedir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const repository = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const skillName = 'keepsake-add-facts';
const source = join(repository, 'skills', skillName);

for (const host of ['.claude', '.codex']) {
  const parent = join(homedir(), host, 'skills');
  const destination = join(parent, skillName);
  const existing = await lstat(destination).catch((error) => {
    if (error.code === 'ENOENT') return undefined;
    throw error;
  });
  if (existing?.isSymbolicLink() || (existing && !existing.isDirectory())) {
    throw new Error(`Skill destination must be a directory: ${destination}`);
  }
  await mkdir(parent, { recursive: true });
  await cp(source, destination, { recursive: true, force: true });
  await writeFile(join(destination, 'repository.json'), `${JSON.stringify({ repository }, null, 2)}\n`);
  console.log(`Installed ${destination}`);
}
