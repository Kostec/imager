// SPDX-License-Identifier: GPL-2.0-or-later

// Dev check of the profile model, no dependency and not part of the app bundle. Run:
//   npx rolldown src/config/profileEditorModel.devtest.ts --format esm --platform node --file <tmp>/model.mjs && node <tmp>/model.mjs
// Expected values come from the spec (stories 10, 14, 18, 23, 24, 25, 33, 34, 35) and from provision::is_empty.

import * as autoconfig from './autoconfig';
import * as model from './profileEditorModel';
import * as wizard from './profileWizard';
import type { AutoconfigConfig } from '../types';

// Loose on purpose: the same file has to run before the model exists, to see it fail.
/* eslint-disable @typescript-eslint/no-explicit-any */
const m = model as any;
const a = autoconfig as any;
const w = wizard as any;

const PLAIN = 'correct horse battery';
const HASH = '$6$salt$hash';
const PSK = 'a'.repeat(64);
const ops = {
  hashPassword: async (plain: string) => `$6$salt$${plain.length}`,
  deriveWifiPsk: async (ssid: string, key: string) => `${ssid.length}:${key.length}`.padEnd(64, '0'),
};
const defaults = { timezone: 'Europe/Moscow', locale: 'ru_RU.UTF-8' };
const form = (config: AutoconfigConfig = {}) =>
  m.toEditorForm({ id: '1', name: 'p', updatedAt: 0, config }, defaults);
const blocked = (f: unknown) => m.firstBlockingGroup(f)?.group ?? null;

const cases: [string, () => void | Promise<void>][] = [];
const test = (name: string, run: () => void | Promise<void>) => cases.push([name, run]);
function same(actual: unknown, expected: unknown, what = '') {
  const [x, y] = [JSON.stringify(actual), JSON.stringify(expected)];
  if (x !== y) throw new Error(`${what} expected ${y}, got ${x}`);
}

test('login follows the spec pattern and refuses reserved names', () => {
  for (const ok of ['pi', '_svc', 'a-b_1', 'a'.repeat(32)]) same(m.isValidUserName(ok), true, ok);
  for (const bad of ['Pi', '1pi', 'a b', 'a'.repeat(33), 'root', 'daemon', 'nobody', 'sshd']) same(m.isValidUserName(bad), false, bad);
});

test('hostname follows RFC 1123', () => {
  for (const ok of ['wukong', 'a-1', '1a', 'a.b']) same(a.isValidHostname(ok), true, ok);
  for (const bad of ['-a', 'a-', 'a_b', 'a b', 'я', 'a'.repeat(64), 'a..b']) same(a.isValidHostname(bad), false, bad);
});

test('Wi-Fi password is 8-63 ASCII or 64 hex', () => {
  for (const ok of ['12345678', 'a'.repeat(63), PSK]) same(a.isWifiKeyValid(ok), true, ok);
  for (const bad of ['1234567', 'g'.repeat(64), 'пароль-длинный']) same(a.isWifiKeyValid(bad), false, bad);
});

test('script over 64 KiB blocks, in bytes, and keeps the text', () => {
  const f = form({ hostname: 'x' });
  f.device.script = 'я'.repeat(32 * 1024 + 1);
  same(blocked(f), 'device');
  same(f.device.script.length, 32 * 1024 + 1);
  f.device.script = 'a'.repeat(64 * 1024);
  same(blocked(f), null);
});

test('password login off without keys blocks; a key clears it', () => {
  const f = form({ userName: 'pi', userPasswordHash: HASH, sshEnabled: true, sshPasswordAuth: false });
  same(blocked(f), 'access');
  f.access.keys = 'ssh-ed25519 AAAA me@host';
  same(blocked(f), null);
});

test('SSH on, no user and root login off blocks: nobody could log in over SSH', () => {
  const f = form({ sshEnabled: true, sshRootLogin: false });
  same(m.accessError(f), 'editor.sshRootNoUser');
  same(blocked(f), 'access');
  f.access.rootLogin = true;
  same(blocked(f), null);
  same(blocked(form({ sshEnabled: true, sshRootLogin: false, userName: 'pi', userPasswordHash: HASH })), null);
  same(blocked(form({ sshEnabled: false })), null);
});

test('the wizard sees a skipped user step, also at the end', () => {
  const f = form({ userName: 'pi', userPasswordHash: HASH, sshEnabled: true, sshRootLogin: false });
  same(w.wizardStepError('access', f, {}), null);
  same(w.wizardStepError('access', f, { user: true }), 'settings.autoconfig.editor.sshRootNoUser');
  same(w.wizardProfileError(f, {}), null);
  same(w.wizardProfileError(f, { user: true }), 'settings.autoconfig.editor.sshRootNoUser');
  same(w.wizardProfileError(f, { user: true, access: true }), null);
});

test('a private key is refused and counts as no key', () => {
  const pem = '-----BEGIN OPENSSH PRIVATE KEY-----\nb3BlbnNzaC1rZXktdjEAAAAA\n-----END OPENSSH PRIVATE KEY-----';
  same(a.countAuthorizedKeys(pem), 0);
  same(a.areAuthorizedKeysValid(pem), false);
  const f = form({ userName: 'pi', userPasswordHash: HASH, sshEnabled: true, sshPasswordAuth: false });
  f.access.keys = pem;
  same(m.accessError(f), 'editor.keysInvalid');
  same(blocked(f), 'access');
  f.access.passwordAuth = true;
  same(blocked(f), 'access', 'with password login on');
});

test('only a line with a key type is a key', () => {
  const keys = [
    'ssh-ed25519 AAAAC3Nza me@host',
    'ssh-rsa AAAAB3Nza==',
    'ecdsa-sha2-nistp256 AAAAE2Vj',
    'sk-ssh-ed25519@openssh.com AAAAGnNr',
    'no-pty ssh-ed25519 AAAAC3Nza',
  ];
  same(a.countAuthorizedKeys(keys.join('\n')), keys.length);
  same(a.areAuthorizedKeysValid(keys.join('\r\n')), true);
  same(a.countAuthorizedKeys('# a comment\nssh-rsa AAAA'), 1);
  same(a.areAuthorizedKeysValid('# a comment\nssh-rsa AAAA'), true);
  for (const bad of ['hello', 'ssh-ed25519', 'ssh-ed25519 not*base64', '[Unit] ssh-rsa AAAA']) same(a.areAuthorizedKeysValid(bad), false, bad);
});

test('a filled key source that was not imported blocks', () => {
  const f = form({ sshEnabled: true });
  f.access.keyUser = 'octocat';
  same(blocked(f), 'access');
});

test('Wi-Fi needs a country and a valid SSID', () => {
  same(blocked(form({ wifiEnabled: true, wifiSsid: 'net' })), 'network');
  same(blocked(form({ wifiEnabled: true, wifiSsid: 'net', wifiCountryCode: 'RU' })), null);
  same(blocked(form({ wifiEnabled: true, wifiSsid: 'я'.repeat(17), wifiCountryCode: 'RU' })), 'network');
});

test('a fixed IP without a mask is an error at the field and stops the editor and the wizard', () => {
  const f = form({ hostname: 'x', useStaticIp: true, staticIp: '192.168.1.50', staticGateway: '192.168.1.1' });
  same(m.editorIpErrors(f.network).mask, 'maskInvalid');
  same(m.firstBlockingGroup(f), { group: 'network', more: 'network' });
  same(w.wizardStepBlocked('network', f), true);
  same(w.wizardProfileError(f, {}), 'settings.autoconfig.maskInvalid', 'going back must not get past the check');
  same(w.wizardProfileError(f, { network: true }), null, 'a skipped step writes nothing');
  f.network.mask = '   ';
  same(blocked(f), 'network', 'spaces are no mask');
  f.network.mask = '255.255.255.0';
  same(m.editorIpErrors(f.network), {});
  same(blocked(f), null);
  same(w.wizardStepBlocked('network', f), false);
  f.network.mask = '';
  f.network.ip = 'auto';
  same(blocked(f), null, 'automatic needs no mask');
  same(w.wizardStepBlocked('network', f), false, 'automatic needs no mask');
});

test('sealing leaves no plain text in the profile and clears the form', async () => {
  const f = form({ userName: 'pi', userPasswordHash: HASH, wifiEnabled: true, wifiSsid: 'net', wifiCountryCode: 'RU' });
  f.secrets = { wifiKey: PLAIN, userPassword: PLAIN, rootPassword: PLAIN };
  const sealed = await m.sealEditorForm(f, ops);
  same(sealed.secrets, { wifiKey: '', userPassword: '', rootPassword: '' });
  const text = JSON.stringify(m.fromEditorModel(sealed));
  same(text.includes(PLAIN), false, 'plain text in the profile');
  same(sealed.user.passwordHash, `$6$salt$${PLAIN.length}`);
  same(sealed.network.psk.length, 64);
});

test('an empty password keeps the stored hash and PSK', async () => {
  const f = form({ userName: 'pi', userPasswordHash: HASH, rootPasswordHash: HASH, wifiEnabled: true, wifiSsid: 'net', wifiPsk: PSK, wifiCountryCode: 'RU' });
  same(blocked(f), null);
  const { config } = m.fromEditorModel(await m.sealEditorForm(f, ops));
  same([config.userPasswordHash, config.rootPasswordHash, config.wifiPsk], [HASH, HASH, PSK]);
});

test('a new SSID needs the password again; the old PSK never goes out', async () => {
  const f = form({ wifiEnabled: true, wifiSsid: 'net', wifiPsk: PSK, wifiCountryCode: 'RU' });
  f.network.ssid = 'other';
  same(blocked(f), 'network');
  same(m.fromEditorModel(f).config.wifiPsk, undefined);
  f.secrets.wifiKey = '12345678';
  same(blocked(f), null);
  const { config } = m.fromEditorModel(await m.sealEditorForm(f, ops));
  same(config.wifiPsk !== PSK && config.wifiPsk.length === 64, true);
});

test('an open network stays open', async () => {
  const f = form({ wifiEnabled: true, wifiSsid: 'net', wifiCountryCode: 'RU' });
  same(blocked(f), null);
  same(m.fromEditorModel(await m.sealEditorForm(f, ops)).config.wifiPsk, undefined);
});

test('only a typed root password makes an otherwise empty form count', () => {
  const f = form({});
  same(m.isFormEmpty(f), true);
  f.secrets.rootPassword = PLAIN;
  same(m.isFormEmpty(f), false);
});

test('an untouched SSH section sends nothing, so the profile stays empty', () => {
  const { config } = m.fromEditorModel(form({}));
  same(a.presetIsEmpty(config), true);
  same(Object.keys(config).filter((k) => k.startsWith('ssh')), []);
});

test('a new profile offers the name wukong, an existing one keeps its own', () => {
  same(m.toEditorForm(null, defaults).device.hostname, 'wukong');
  same(form({}).device.hostname, '');
  same(m.fromEditorModel(m.toEditorForm(null, defaults)).config.hostname, 'wukong');
});

test('profile round trip keeps every field', () => {
  const config: AutoconfigConfig = {
    ethernetEnabled: true, wifiEnabled: true, wifiSsid: ' net ', wifiPsk: PSK, wifiCountryCode: 'RU', wifiHidden: true,
    useStaticIp: true, staticIp: '192.168.1.50', staticMask: '255.255.255.0', staticGateway: '192.168.1.1', staticDns: '8.8.8.8',
    userName: 'pi', userPasswordHash: HASH, userRealName: 'P I', userShell: 'zsh', rootPasswordHash: HASH,
    sshEnabled: true, sshPasswordAuth: false, sshRootLogin: true, sshAuthorizedKeys: 'ssh-ed25519 AAAA',
    locale: 'ru_RU.UTF-8', timezone: 'Europe/Moscow', keyboardLayout: 'ru',
    hostname: 'wukong', overlays: ['i2c0', 'uart1'], userScript: 'echo hi',
  };
  const out = m.fromEditorModel(form(config)).config;
  const sorted = (o: object) => Object.fromEntries(Object.entries(o).sort(([x], [y]) => x.localeCompare(y)));
  same(sorted(out), sorted(config));
});

test('presetIsEmpty twins provision::is_empty', () => {
  const empty: AutoconfigConfig[] = [
    {}, { ethernetEnabled: true }, { wifiEnabled: false, wifiSsid: 'x' }, { useStaticIp: false, staticIp: '1.1.1.1' },
    { hostname: '  ' }, { overlays: [] }, { userScript: ' \n' },
  ];
  const filled: AutoconfigConfig[] = [
    { wifiEnabled: true }, { useStaticIp: true }, { hostname: 'x' }, { keyboardLayout: 'ru' }, { sshEnabled: false },
    { sshPasswordAuth: true }, { sshRootLogin: false }, { sshAuthorizedKeys: 'k' }, { overlays: ['i2c0'] }, { userScript: 'x' },
    { rootPasswordHash: HASH }, { userShell: 'zsh' },
  ];
  for (const c of empty) same(a.presetIsEmpty(c), true, JSON.stringify(c));
  for (const c of filled) same(a.presetIsEmpty(c), false, JSON.stringify(c));
});

test('root/1234 warning: no user and no root password, with Wi-Fi or SSH', () => {
  same(a.defaultRootExposed({ wifiEnabled: true }), true);
  same(a.defaultRootExposed({ sshEnabled: true }), true);
  same(a.defaultRootExposed({ hostname: 'x', sshEnabled: false }), false);
  same(a.defaultRootExposed({ wifiEnabled: true, userName: 'pi' }), false);
  same(a.defaultRootExposed({ wifiEnabled: true, rootPasswordHash: HASH }), false);
});

test('the root warning stays true whatever SSH does', () => {
  same(a.defaultRootExposed({ wifiEnabled: true, sshEnabled: false }), true);
  same(a.defaultRootExposed({ sshEnabled: true, sshRootLogin: true }), true);
});

test('connect hint names the user and host, or nothing when SSH is off', () => {
  same(a.sshCommand({ sshEnabled: true, sshRootLogin: false }), null);
  same(a.sshCommand({ hostname: 'wukong', sshRootLogin: false, userName: 'pi' }), 'ssh pi@wukong.local');
  same(a.sshCommand({ userName: 'pi', hostname: 'wukong' }), 'ssh pi@wukong.local');
  same(a.sshCommand({ hostname: 'wukong' }), 'ssh root@wukong.local');
  same(a.sshCommand({ userName: 'pi' }), 'ssh pi@wukongpi.local');
  same(a.sshCommand({ userName: 'pi', useStaticIp: true, staticIp: '192.168.1.50' }), 'ssh pi@192.168.1.50');
  same(a.sshCommand({ userName: 'pi', sshEnabled: false }), null);
});

test('settings availability per image answer', () => {
  same(a.settingsAvailable({ kind: 'supported' }), true);
  same(a.settingsAvailable({ kind: 'deferred' }), true);
  for (const kind of ['notExt4', 'noFirstrunService', 'alreadyBooted', 'unreadable', 'catalogRefused']) same(a.settingsAvailable({ kind }), false, kind);
  same(a.settingsAvailable({ kind: 'unsupportedFs', reason: 'x' }), false);
  same(a.settingsAvailable(undefined), false);
});

test('imported keys merge without duplicates', () => {
  same(a.mergeAuthorizedKeys('k1\nk2', 'k2\r\nk3\n'), 'k1\nk2\nk3');
});

let failed = 0;
for (const [name, run] of cases) {
  try {
    await run();
    console.log(`ok    ${name}`);
  } catch (error) {
    failed += 1;
    console.log(`FAIL  ${name}: ${(error as Error).message}`);
  }
}
console.log(`${cases.length - failed} passed, ${failed} failed`);
if (failed) throw new Error('model check failed');
