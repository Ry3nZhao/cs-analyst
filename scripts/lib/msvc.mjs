import { execFileSync, spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';

/**
 * Resolves the MSVC build environment (link.exe, LIB, INCLUDE, ...) so cargo can
 * link on Windows without launching a "Developer PowerShell".
 *
 * Why this exists: rustc auto-detects the MSVC toolchain via vswhere, but newer
 * Visual Studio installs (e.g. VS 18) are not reported by older vswhere builds,
 * so `cargo build` fails with "linker `link.exe` not found" from a plain shell.
 * We locate vcvars64.bat ourselves (vswhere first, then a directory scan) and
 * capture the environment it sets.
 */

let cached;

function installerDir() {
  const programFilesX86 = process.env['ProgramFiles(x86)'] ?? '';
  return path.join(programFilesX86, 'Microsoft Visual Studio', 'Installer');
}

function vswhereVcvars() {
  const vswhere = path.join(installerDir(), 'vswhere.exe');
  if (!fs.existsSync(vswhere)) {
    return null;
  }
  try {
    const out = execFileSync(
      vswhere,
      [
        '-latest',
        '-prerelease',
        '-products',
        '*',
        '-requires',
        'Microsoft.VisualStudio.Component.VC.Tools.x86.x64',
        '-property',
        'installationPath',
      ],
      { encoding: 'utf8' },
    ).trim();
    if (out) {
      const bat = path.join(out, 'VC', 'Auxiliary', 'Build', 'vcvars64.bat');
      if (fs.existsSync(bat)) {
        return bat;
      }
    }
  } catch {
    // vswhere failed or found nothing; fall through to the directory scan.
  }
  return null;
}

function scanForVcvars() {
  const roots = [
    process.env.ProgramFiles && path.join(process.env.ProgramFiles, 'Microsoft Visual Studio'),
    process.env['ProgramFiles(x86)'] &&
      path.join(process.env['ProgramFiles(x86)'], 'Microsoft Visual Studio'),
  ].filter(Boolean);

  for (const root of roots) {
    if (!fs.existsSync(root)) {
      continue;
    }
    // Layout: <root>/<version>/<edition>/VC/Auxiliary/Build/vcvars64.bat
    for (const version of fs.readdirSync(root)) {
      const versionDir = path.join(root, version);
      if (!fs.statSync(versionDir).isDirectory()) {
        continue;
      }
      for (const edition of fs.readdirSync(versionDir)) {
        const bat = path.join(
          versionDir,
          edition,
          'VC',
          'Auxiliary',
          'Build',
          'vcvars64.bat',
        );
        if (fs.existsSync(bat)) {
          return bat;
        }
      }
    }
  }
  return null;
}

function findVcvars() {
  if (process.env.CS_DEMO_ANALYST_VCVARS) {
    return process.env.CS_DEMO_ANALYST_VCVARS;
  }
  return vswhereVcvars() || scanForVcvars();
}


/**
 * Returns env vars to overlay onto the cargo child process so the MSVC linker is
 * found. On non-Windows, or when already inside a Developer environment, returns
 * an empty object. Result is cached for the process lifetime.
 */
export function msvcEnv() {
  if (cached) {
    return cached;
  }
  if (process.platform !== 'win32') {
    cached = {};
    return cached;
  }
  // Already inside a Developer PowerShell / VS prompt — nothing to do.
  if (process.env.VCINSTALLDIR) {
    cached = {};
    return cached;
  }

  const vcvars = findVcvars();
  if (!vcvars) {
    console.warn(
      '[msvc] Could not locate vcvars64.bat. If cargo fails with "linker `link.exe` not found", ' +
        'set CS_DEMO_ANALYST_VCVARS to its full path or run from a Developer PowerShell.',
    );
    cached = {};
    return cached;
  }

  // Run `call <vcvars> & set` and parse the resulting environment.
  // Two Windows-specific details matter here:
  //   1. Pass <vcvars> as its own argv entry (not embedded in a quoted /c
  //      string) so Node quotes the spaces in the path correctly. Embedding
  //      quotes in the string gets mangled by cmd.exe arg handling.
  //   2. Prepend the VS Installer dir to PATH so vcvarsall.bat can find
  //      vswhere.exe. On VS 18 the install lives under "Program Files" but
  //      vswhere only exists under "Program Files (x86)", so vcvarsall's
  //      relative lookup fails ("'vswhere.exe' is not recognized") and the
  //      environment never gets LIB/INCLUDE/VCINSTALLDIR set.
  // `&` (not `&&`) ensures `set` runs regardless of vcvars' exit code; reading
  // spawnSync's stdout avoids throwing away output on a non-zero exit.
  const spawnEnv = {
    ...process.env,
    PATH: `${installerDir()};${process.env.PATH || ''}`,
  };
  const callArgs = ['/c', 'call', vcvars, '&', 'set'];
  const result = spawnSync('cmd.exe', callArgs, {
    encoding: 'utf8',
    maxBuffer: 1024 * 1024 * 16,
    env: spawnEnv,
  });
  const parsed = {};
  for (const line of (result.stdout || '').split(/\r?\n/)) {
    const eq = line.indexOf('=');
    if (eq > 0) {
      parsed[line.slice(0, eq)] = line.slice(eq + 1);
    }
  }
  if (parsed.VCINSTALLDIR) {
    cached = parsed;
    const toolset = parsed.VCToolsVersion ? ` (toolset ${parsed.VCToolsVersion})` : '';
    console.log(`[msvc] Loaded MSVC environment from ${vcvars}${toolset}`);
  } else {
    console.warn(
      `[msvc] Ran ${vcvars} but VCINSTALLDIR was not set; MSVC environment may be incomplete.`,
    );
    cached = {};
  }
  return cached;
}
