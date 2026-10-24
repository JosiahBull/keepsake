#!/usr/bin/env node
import { existsSync, readFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

function isKeepsake(directory) {
  try {
    return existsSync(join(directory, 'Cargo.toml'))
      && JSON.parse(readFileSync(join(directory, 'package.json'), 'utf8')).name === 'keepsake';
  } catch {
    return false;
  }
}

function findRepository() {
  if (process.env.KEEPSAKE_REPO) return resolve(process.env.KEEPSAKE_REPO);
  let directory = dirname(fileURLToPath(import.meta.url));
  while (true) {
    if (isKeepsake(directory)) return directory;
    const parent = dirname(directory);
    if (parent === directory) break;
    directory = parent;
  }
  try {
    const configuration = JSON.parse(readFileSync(join(dirname(fileURLToPath(import.meta.url)), '..', 'repository.json'), 'utf8'));
    return resolve(configuration.repository);
  } catch {
    return undefined;
  }
}

const repository = findRepository();
if (!repository || !isKeepsake(repository)) {
  console.error('Keepsake repository unavailable. Set KEEPSAKE_REPO to the checkout path or reinstall the skill.');
  process.exit(1);
}

const result = spawnSync('cargo', [
  'run', '--quiet', '--manifest-path', join(repository, 'Cargo.toml'),
  '-p', 'keepsake-cli', '--bin', 'keepsake', '--', ...process.argv.slice(2),
], { env: { ...process.env, DATABASE_URL: process.env.DATABASE_URL ?? `sqlite:${join(repository, 'data', 'keepsake.db')}` }, stdio: 'inherit' });

if (result.error) {
  console.error(`Could not launch Keepsake: ${result.error.message}`);
  process.exit(1);
}
if (result.signal) {
  console.error(`Keepsake stopped by ${result.signal}.`);
  process.exit(1);
}
process.exit(result.status ?? 1);
