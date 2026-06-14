#!/usr/bin/env node
import fs from 'node:fs';
import path from 'node:path';
import { root } from '../lib/paths.mjs';

const version = JSON.parse(fs.readFileSync(path.join(root, 'package.json'), 'utf8')).version;

const tauriConfPath = path.join(root, 'crates', 'tauri-app', 'src-tauri', 'tauri.conf.json');
const tauriConf = JSON.parse(fs.readFileSync(tauriConfPath, 'utf8'));
tauriConf.version = version;
fs.writeFileSync(tauriConfPath, `${JSON.stringify(tauriConf, null, 2)}\n`);

const cargoTomlPath = path.join(root, 'crates', 'tauri-app', 'src-tauri', 'Cargo.toml');
let cargoToml = fs.readFileSync(cargoTomlPath, 'utf8');
cargoToml = cargoToml.replace(/^version = ".*"$/m, `version = "${version}"`);
fs.writeFileSync(cargoTomlPath, cargoToml);

console.log(`Synced version ${version} to Tauri/Cargo`);
