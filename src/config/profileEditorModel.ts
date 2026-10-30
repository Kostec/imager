// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2026 Daniele Briguglio, superkali@armbian.com

import type { AutoconfigConfig, AutoconfigProfile, ProfileBoard, UserShell } from '../types';
import { isHttpUrl, trimmedOrUndefined } from '../utils';
import type { TFn } from '../utils/errorUtils';
import { AUTOCONFIG, UI } from './constants';
import {
  areAuthorizedKeysValid,
  countAuthorizedKeys,
  isSsidTooLong,
  isUserScriptTooLarge,
  isValidHostname,
  isValidKeyUser,
  isWifiKeyValid,
  presetIsEmpty,
  profileBoards,
  staticIpConfigErrors,
  trimStaticIp,
  type SshKeySource,
} from './autoconfig';

export type BoardScope = 'all' | 'some';
export type NetMode = 'cable' | 'wifi' | 'both';
export type IpMode = 'auto' | 'fixed';
export type UserMode = 'now' | 'later';
export type RegionChoice = 'keep' | 'choose';
export type SshChoice = 'on' | 'off';

export const EDITOR_GROUPS = ['name', 'network', 'user', 'access', 'region', 'device'] as const;
export type EditorGroup = (typeof EDITOR_GROUPS)[number];
export type GroupStatus = 'set' | 'attn' | 'none';
export type MoreKey = 'network' | 'user' | 'access';

export const PROFILE_EDITOR = {
  /** Board tile in the editor header, px */
  TILE_SIZE: 96,
  SEPARATOR: UI.SUMMARY_SEPARATOR,
} as const;

// As is_user_name in src-tauri/src/provision/bundle.rs.
const USER_NAME_PATTERN = /^[a-z_][a-z0-9_-]{0,31}$/;

// Accounts Debian and the image already have; adduser would refuse them on the board, after the card is written.
const RESERVED_USER_NAMES: ReadonlySet<string> = new Set([
  'root', 'daemon', 'bin', 'sys', 'sync', 'games', 'man', 'lp', 'mail', 'news', 'uucp', 'proxy', 'www-data', 'backup', 'list', 'irc',
  'gnats', 'nobody', '_apt', 'systemd-network', 'systemd-resolve', 'systemd-timesync', 'systemd-coredump', 'messagebus', 'sshd',
  'avahi', 'avahi-autoipd', 'chrony', 'dnsmasq', 'tss', 'polkitd', 'pulse', 'rtkit', 'usbmux', 'admin', 'sudo', 'ssh', 'tty', 'disk',
]);

export function isValidUserName(name: string): boolean {
  const login = name.trim();
  return USER_NAME_PATTERN.test(login) && !RESERVED_USER_NAMES.has(login);
}

/** What goes to the profile: hashes and the derived Wi-Fi key, never what was typed. */
export interface EditorModel {
  name: string;
  scope: BoardScope;
  boards: ProfileBoard[];
  /** `apply` false writes nothing, so the image keeps its own defaults; the values stay for when it is applied again */
  network: {
    apply: boolean;
    mode: NetMode;
    ssid: string;
    /** The derived key is only good for the network it was derived for */
    psk: string;
    pskSsid: string;
    hidden: boolean;
    country: string;
    ip: IpMode;
    address: string;
    mask: string;
    gateway: string;
    dns: string;
  };
  user: { mode: UserMode; name: string; passwordHash: string; realName: string; shell: UserShell | '' };
  /** `source` and the two fields after it are only the import form; what counts is `keys` */
  access: {
    apply: boolean;
    enabled: boolean;
    passwordAuth: boolean;
    rootLogin: boolean;
    keys: string;
    source: SshKeySource;
    keyUser: string;
    keyLink: string;
    rootPasswordHash: string;
  };
  region: { apply: boolean; locale: string; timezone: string; keyboard: string };
  device: { hostname: string; overlays: string[]; script: string };
}

/** What was typed and not sealed yet; lives in the form state only and is never saved. */
export interface EditorSecrets {
  wifiKey: string;
  userPassword: string;
  rootPassword: string;
}

export interface EditorForm extends EditorModel {
  secrets: EditorSecrets;
}

export interface SecretOps {
  hashPassword: (plain: string) => Promise<string>;
  deriveWifiPsk: (ssid: string, passphrase: string) => Promise<string>;
}

// A new object each time: a shared one would carry what is typed in one form into the next.
const noSecrets = (): EditorSecrets => ({ wifiKey: '', userPassword: '', rootPassword: '' });

export const hasWifi = (mode: NetMode) => mode !== 'cable';
const hasCable = (mode: NetMode) => mode !== 'wifi';

export function toEditorForm(profile: AutoconfigProfile | null, defaults: { timezone: string; locale: string }): EditorForm {
  const c: AutoconfigConfig = profile?.config ?? {};
  const boards = profile ? profileBoards(profile) : [];
  const net = !!(c.ethernetEnabled || c.wifiEnabled || c.useStaticIp);
  const sshSet = c.sshEnabled !== undefined || c.sshPasswordAuth !== undefined || c.sshRootLogin !== undefined || !!c.sshAuthorizedKeys;
  return {
    name: profile?.name ?? '',
    scope: boards.length ? 'some' : 'all',
    boards,
    network: {
      apply: net,
      mode: c.wifiEnabled ? (c.ethernetEnabled ? 'both' : 'wifi') : 'cable',
      ssid: c.wifiSsid ?? '',
      psk: c.wifiPsk ?? '',
      pskSsid: c.wifiPsk ? (c.wifiSsid ?? '') : '',
      hidden: !!c.wifiHidden,
      country: c.wifiCountryCode ?? (profile ? '' : localeCountry(defaults.locale)),
      ip: c.useStaticIp ? 'fixed' : 'auto',
      address: c.staticIp ?? '',
      mask: c.staticMask ?? '',
      gateway: c.staticGateway ?? '',
      dns: c.staticDns ?? '',
    },
    user: {
      mode: c.userName?.trim() ? 'now' : 'later',
      name: c.userName ?? '',
      passwordHash: c.userPasswordHash ?? '',
      realName: c.userRealName ?? '',
      shell: c.userShell ?? '',
    },
    access: {
      apply: sshSet,
      enabled: c.sshEnabled ?? true,
      passwordAuth: c.sshPasswordAuth ?? true,
      rootLogin: c.sshRootLogin ?? false,
      keys: c.sshAuthorizedKeys ?? '',
      source: 'github',
      keyUser: '',
      keyLink: '',
      rootPasswordHash: c.rootPasswordHash ?? '',
    },
    region: {
      apply: !!(c.locale || c.timezone || c.keyboardLayout),
      locale: c.locale ?? (profile ? '' : defaults.locale),
      timezone: c.timezone ?? (profile ? '' : defaults.timezone),
      keyboard: c.keyboardLayout ?? '',
    },
    device: {
      hostname: c.hostname ?? (profile ? '' : AUTOCONFIG.HOSTNAME_PROPOSED),
      overlays: c.overlays ?? [],
      script: c.userScript ?? '',
    },
    secrets: noSecrets(),
  };
}

/** Turns what was typed into hashes and the derived key, and forgets it. An empty field keeps what is stored. */
export async function sealEditorForm(form: EditorForm, ops: SecretOps): Promise<EditorForm> {
  const { network: n, user: u, access: a, secrets } = form;
  const psk = n.apply && hasWifi(n.mode) && secrets.wifiKey ? await ops.deriveWifiPsk(n.ssid, secrets.wifiKey) : null;
  const userHash = u.mode === 'now' && secrets.userPassword ? await ops.hashPassword(secrets.userPassword) : null;
  const rootHash = secrets.rootPassword ? await ops.hashPassword(secrets.rootPassword) : null;
  return {
    ...form,
    network: psk === null ? n : { ...n, psk, pskSsid: n.ssid },
    user: userHash === null ? u : { ...u, passwordHash: userHash },
    access: rootHash === null ? a : { ...a, rootPasswordHash: rootHash },
    secrets: noSecrets(),
  };
}

// The SSID is kept as typed: a leading or trailing space can be part of one.
const asTyped = (value: string) => (value ? value : undefined);

function editorConfig(m: EditorModel): AutoconfigConfig {
  const config: AutoconfigConfig = {};
  const n = m.network;
  if (n.apply) {
    config.ethernetEnabled = hasCable(n.mode);
    config.wifiEnabled = hasWifi(n.mode);
    if (hasWifi(n.mode)) {
      config.wifiSsid = asTyped(n.ssid);
      // A key derived for another network would never connect, so it does not travel with the new name.
      config.wifiPsk = n.pskSsid === n.ssid ? asTyped(n.psk) : undefined;
      config.wifiCountryCode = trimmedOrUndefined(n.country);
      config.wifiHidden = n.hidden || undefined;
    }
    config.useStaticIp = n.ip === 'fixed';
    if (n.ip === 'fixed') {
      Object.assign(config, trimStaticIp({ staticIp: n.address, staticMask: n.mask, staticGateway: n.gateway, staticDns: n.dns }));
    }
  }
  if (m.user.mode === 'now') {
    config.userName = trimmedOrUndefined(m.user.name);
    config.userPasswordHash = asTyped(m.user.passwordHash);
    config.userRealName = trimmedOrUndefined(m.user.realName);
    config.userShell = m.user.shell || undefined;
  }
  config.rootPasswordHash = asTyped(m.access.rootPasswordHash);
  if (m.access.apply) {
    config.sshEnabled = m.access.enabled;
    if (m.access.enabled) {
      config.sshPasswordAuth = m.access.passwordAuth;
      config.sshRootLogin = m.access.rootLogin;
      config.sshAuthorizedKeys = trimmedOrUndefined(m.access.keys);
    }
  }
  if (m.region.apply) {
    config.locale = trimmedOrUndefined(m.region.locale);
    config.timezone = trimmedOrUndefined(m.region.timezone);
    config.keyboardLayout = trimmedOrUndefined(m.region.keyboard);
  }
  config.hostname = trimmedOrUndefined(m.device.hostname);
  config.overlays = m.device.overlays.length ? m.device.overlays : undefined;
  config.userScript = m.device.script.trim() ? m.device.script : undefined;
  return Object.fromEntries(Object.entries(config).filter(([, v]) => v !== undefined)) as AutoconfigConfig;
}

export function fromEditorModel(m: EditorModel): Pick<AutoconfigProfile, 'name' | 'config' | 'boards'> {
  return {
    name: m.name.trim(),
    config: editorConfig(m),
    boards: m.scope === 'some' && m.boards.length ? m.boards : undefined,
  };
}

/** Nothing would reach the card. A root password still in the field counts: it becomes a hash on saving. */
export const isFormEmpty = (form: EditorForm) => presetIsEmpty(editorConfig(form)) && !form.secrets.rootPassword;

const keySourceValue = (a: EditorModel['access']) => (a.source === 'link' ? a.keyLink : a.keyUser).trim();

/** i18n key (under settings.autoconfig) when the import field is malformed; empty is fine */
export function keySourceFormatError(a: EditorModel['access']): string | null {
  const value = keySourceValue(a);
  if (!value) return null;
  if (a.source === 'link') return isHttpUrl(value) ? null : 'keys.invalidLink';
  return isValidKeyUser(a.source, value) ? null : 'wizard.keyUserInvalid';
}

/** i18n key (under settings.autoconfig) of what is wrong with the SSH settings, or null */
export function accessError(a: EditorModel['access']): string | null {
  if (!a.apply || !a.enabled) return null;
  const format = keySourceFormatError(a);
  if (format) return format;
  // Typed but not imported: saving would silently lose the keys the user meant to add.
  if (keySourceValue(a)) return 'editor.keysNotImported';
  if (!areAuthorizedKeysValid(a.keys)) return 'editor.keysInvalid';
  return !a.passwordAuth && !countAuthorizedKeys(a.keys) ? 'editor.sshNoWayIn' : null;
}

export function editorIpErrors(n: EditorModel['network']) {
  return n.ip === 'fixed' ? staticIpConfigErrors({ staticIp: n.address, staticMask: n.mask, staticGateway: n.gateway, staticDns: n.dns }) : {};
}

/** i18n key (under settings.autoconfig) of what is wrong with the SSID, or null */
export function ssidError(n: EditorModel['network']): string | null {
  if (!n.ssid.trim()) return 'wizard.ssidRequired';
  return isSsidTooLong(n.ssid) ? 'editor.ssidTooLong' : null;
}

/** i18n key (under settings.autoconfig) of what is wrong with the Wi-Fi password, or null; empty means an open network */
export function wifiKeyError(form: EditorForm): string | null {
  const { wifiKey } = form.secrets;
  if (wifiKey) return isWifiKeyValid(wifiKey) ? null : 'wifiKeyLength';
  return form.network.psk && form.network.pskSsid !== form.network.ssid ? 'editor.wifiKeyAgain' : null;
}

const wifiBlockedInMain = (form: EditorForm) =>
  hasWifi(form.network.mode) && (!!ssidError(form.network) || !!wifiKeyError(form));

function networkBlocked(form: EditorForm): boolean {
  const n = form.network;
  if (!n.apply) return false;
  if (wifiBlockedInMain(form) || (hasWifi(n.mode) && !n.country.trim())) return true;
  if (n.ip !== 'fixed') return false;
  return !n.address.trim() || Object.keys(editorIpErrors(n)).length > 0;
}

/** i18n key (under settings.autoconfig) of what stops this account, or null */
export function userError(form: EditorForm): string | null {
  const u = form.user;
  if (u.mode !== 'now') return null;
  if (!u.name.trim()) return 'wizard.usernameRequired';
  if (!isValidUserName(u.name)) return 'editor.userInvalid';
  return form.secrets.userPassword || u.passwordHash ? null : 'editor.passwordRequired';
}

export const hostnameError = (d: EditorModel['device']) =>
  d.hostname.trim() && !isValidHostname(d.hostname.trim()) ? 'editor.hostnameInvalid' : null;

export const scriptError = (d: EditorModel['device']) => (isUserScriptTooLarge(d.script) ? 'editor.scriptTooLarge' : null);

const deviceBlocked = (d: EditorModel['device']) => !!hostnameError(d) || !!scriptError(d);

export interface ProfileProgress {
  status: Record<EditorGroup, GroupStatus>;
  done: number;
  total: number;
}

/** Per group: set, needs attention, or left at the image's defaults */
export function profileProgress(form: EditorForm): ProfileProgress {
  const { network: n, access: a, region: r, device: d } = form;
  const anyRegion = r.locale || r.timezone || r.keyboard;
  const status: Record<EditorGroup, GroupStatus> = {
    name: form.name.trim() && (form.scope === 'all' || form.boards.length) ? 'set' : 'attn',
    network: !n.apply ? 'none' : networkBlocked(form) ? 'attn' : 'set',
    user: form.user.mode === 'later' ? 'none' : userError(form) ? 'attn' : 'set',
    access: accessError(a) ? 'attn' : a.apply || a.rootPasswordHash || form.secrets.rootPassword ? 'set' : 'none',
    region: r.apply && anyRegion ? 'set' : 'none',
    device: deviceBlocked(d) ? 'attn' : d.hostname.trim() || d.overlays.length || d.script.trim() ? 'set' : 'none',
  };
  const done = EDITOR_GROUPS.filter((g) => status[g] === 'set').length;
  return { status, done, total: EDITOR_GROUPS.length };
}

/** The first group that stops a save, with the folded "More options" that holds the problem; null when it can save */
export function firstBlockingGroup(form: EditorForm): { group: EditorGroup; more?: MoreKey; empty?: boolean } | null {
  if (!form.name.trim() || (form.scope === 'some' && !form.boards.length)) return { group: 'name' };
  if (networkBlocked(form)) return { group: 'network', more: wifiBlockedInMain(form) ? undefined : 'network' };
  if (userError(form)) return { group: 'user' };
  if (accessError(form.access)) return { group: 'access' };
  if (deviceBlocked(form.device)) return { group: 'device' };
  // A profile that leaves every area at the image's defaults would write nothing
  if (isFormEmpty(form)) return { group: 'network', empty: true };
  return null;
}

/** i18n key of why the profile cannot be saved (or created) yet, from the first blocking field; null when it can */
export function blockingReason(form: EditorForm, isNew: boolean): string | null {
  const blocking = firstBlockingGroup(form);
  if (!blocking) return null;
  const key = !form.name.trim() ? 'Name' : blocking.group === 'name' ? 'Board' : blocking.empty ? 'Something' : 'Fix';
  if (isNew && key === 'Name') return 'settings.autoconfig.editor.addNameToCreate';
  return `settings.autoconfig.editor.${isNew ? 'create' : 'save'}Needs${key}`;
}

export function sameModel(a: EditorForm, b: EditorForm): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

/** A country named in the UI language; the English name where the WebView has no Intl.DisplayNames */
export function countryName(code: string, englishName: string, uiLanguage: string): string {
  try {
    return new Intl.DisplayNames([uiLanguage], { type: 'region' }).of(code) ?? englishName;
  } catch {
    return englishName;
  }
}

/** The country part of a glibc locale ("it_IT.UTF-8" gives IT), or '' for C.UTF-8 */
export function localeCountry(locale: string): string {
  return /^[a-z]+_([A-Z]{2})\b/.exec(locale)?.[1] ?? '';
}

export type FactIcon = 'cable' | 'wifi' | 'user' | 'keys' | 'lock' | 'globe' | 'device';
/** ok: applied as shown; plain: the image decides */
type FactTone = 'ok' | 'plain';
export interface ProfileFact {
  icon: FactIcon;
  text: string;
  tone: FactTone;
}

/** What the board does at first boot with this profile, one line per area */
export function profileFacts(m: EditorModel, t: TFn): ProfileFact[] {
  const sep = PROFILE_EDITOR.SEPARATOR;
  const n = m.network;
  const address = n.ip === 'fixed' ? n.address.trim() : '';
  const join = (parts: (string | false)[]) => parts.filter(Boolean).join(sep);
  const netText =
    n.mode === 'cable'
      ? join([t('settings.autoconfig.wizard.network.cable'), address || t('settings.autoconfig.editor.factDhcp')])
      : n.mode === 'wifi'
        ? join([t('settings.autoconfig.wizard.network.wifi'), n.ssid.trim(), address])
        : address
          ? t('settings.autoconfig.editor.factBothFixed', { address })
          : t('settings.autoconfig.editor.factCableWifi');

  const userNow = m.user.mode === 'now' && !!m.user.name.trim();
  const a = m.access;
  const keyCount = countAuthorizedKeys(a.keys);
  let access: ProfileFact;
  if (!a.apply) access = { icon: 'lock', text: t('settings.autoconfig.armbianDefaults'), tone: 'plain' };
  else if (!a.enabled) access = { icon: 'lock', text: t('settings.autoconfig.editor.factSshOff'), tone: 'ok' };
  else if (keyCount) access = { icon: 'keys', text: t('settings.autoconfig.editor.factKeys', { count: keyCount }), tone: 'ok' };
  else access = { icon: 'lock', text: t('settings.autoconfig.wizard.access.passwordOnly'), tone: 'ok' };

  // An area left unapplied writes nothing, so it reads as the image default it keeps
  const region: ProfileFact = !m.region.apply
    ? { icon: 'globe', text: t('settings.autoconfig.armbianDefaults'), tone: 'plain' }
    : {
        icon: 'globe',
        text: m.region.timezone || m.region.locale || m.region.keyboard || t('settings.autoconfig.editor.factTzUnset'),
        tone: 'ok',
      };
  const network: ProfileFact = n.apply
    ? { icon: n.mode === 'cable' ? 'cable' : 'wifi', text: netText, tone: 'ok' }
    : { icon: 'wifi', text: t('settings.autoconfig.armbianDefaults'), tone: 'plain' };

  const d = m.device;
  const hostname = d.hostname.trim();
  const deviceText = join([
    hostname || t('settings.autoconfig.editor.factHostImage', { name: AUTOCONFIG.HOSTNAME_OF_IMAGE }),
    d.overlays.length > 0 && d.overlays.join(', '),
    !!d.script.trim() && t('settings.autoconfig.editor.factScript'),
  ]);
  const customized = !!hostname || d.overlays.length > 0 || !!d.script.trim();

  return [
    network,
    {
      icon: 'user',
      text: userNow ? t('settings.autoconfig.editor.factUser', { name: m.user.name.trim() }) : t('settings.autoconfig.wizard.user.later'),
      tone: userNow ? 'ok' : 'plain',
    },
    access,
    region,
    { icon: 'device', text: deviceText, tone: customized ? 'ok' : 'plain' },
  ];
}

export interface ProfileSpec extends ProfileFact {
  /** i18n key of the area's label */
  label: string;
}

/** A saved profile's first-boot facts for the list card, in reading order; under its label the user is just the account */
export function profileSpec(profile: AutoconfigProfile, t: TFn): ProfileSpec[] {
  const model = toEditorForm(profile, { timezone: '', locale: '' });
  const [network, user, access, region, device] = profileFacts(model, t);
  const account = model.user.mode === 'now' ? model.user.name.trim() : '';
  return [
    { ...network, label: 'settings.autoconfig.wizard.steps.network' },
    { ...user, text: account || user.text, label: 'settings.autoconfig.wizard.steps.user' },
    { ...region, label: 'settings.autoconfig.wizard.steps.locale' },
    { ...access, label: 'settings.autoconfig.wizard.steps.access' },
    { ...device, label: 'settings.autoconfig.wizard.steps.device' },
  ];
}

export function profileScopeLabel(boards: readonly ProfileBoard[], t: TFn): string {
  if (boards.length === 0) return t('settings.profiles.allBoards');
  if (boards.length === 1) return boards[0].name;
  return t('settings.profiles.boardCount', { count: boards.length });
}
