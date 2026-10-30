// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2026 Daniele Briguglio, superkali@armbian.com

import type {
  AutoconfigConfig,
  AutoconfigProfile,
  ImageSettingsSupport,
  ProfileBoard,
  SshKeySource as KeyLookupSource,
  UserShell,
} from '../types';
import { staticIpErrors, trimmedOrUndefined, type StaticIpErrors } from '../utils';
import type { TFn } from '../utils/errorUtils';
import { AUTOCONFIG } from './constants';

/** Example values shown as input placeholders in the profile editor */
export const AUTOCONFIG_PLACEHOLDERS = {
  WIFI_SSID: 'MyHomeWiFi',
  STATIC_IP: '192.168.1.50',
  STATIC_MASK: '255.255.255.0',
  STATIC_GATEWAY: '192.168.1.1',
  STATIC_DNS: '8.8.8.8, 1.1.1.1',
  SSH_KEYS_URL: 'https://github.com/username.keys',
  USER_NAME: 'wukong',
  USER_REAL_NAME: 'Wukong User',
  HOSTNAME: AUTOCONFIG.HOSTNAME_PROPOSED,
  SSH_KEYS: 'ssh-ed25519 AAAA... you@laptop',
  USER_SCRIPT: '#!/bin/bash\napt-get install -y htop',
} as const;

/** Offered as checkboxes; the board's own `usbhost2 usbhost3` stay on whatever is picked here */
export const BOARD_OVERLAYS: readonly string[] = ['i2c0', 'i2c1', 'spi-spidev', 'uart1', 'uart2', 'uart3'];

/** Console keyboard layouts (XKB names) */
export const KEYBOARD_LAYOUTS: readonly string[] = [
  'us', 'gb', 'de', 'fr', 'es', 'it', 'pt', 'br', 'nl', 'se', 'no', 'dk', 'fi', 'pl', 'cz', 'hu', 'ro', 'gr', 'tr', 'ru', 'ua', 'jp', 'kr',
];

/** Login shells offered for the first user. */
export const USER_SHELLS: readonly UserShell[] = ['bash', 'zsh'];

/** Convenience shortlist of common UTF-8 locales; the editor also allows free text. */
export const COMMON_LOCALES: readonly string[] = [
  'en_US.UTF-8',
  'en_GB.UTF-8',
  'de_DE.UTF-8',
  'es_ES.UTF-8',
  'fr_FR.UTF-8',
  'it_IT.UTF-8',
  'nl_NL.UTF-8',
  'pl_PL.UTF-8',
  'pt_PT.UTF-8',
  'pt_BR.UTF-8',
  'ru_RU.UTF-8',
  'sv_SE.UTF-8',
  'tr_TR.UTF-8',
  'uk_UA.UTF-8',
  'ja_JP.UTF-8',
  'ko_KR.UTF-8',
  'zh_CN.UTF-8',
  'zh_TW.UTF-8',
  'hr_HR.UTF-8',
  'sl_SI.UTF-8',
  'cs_CZ.UTF-8',
  'fi_FI.UTF-8',
  'da_DK.UTF-8',
  'nb_NO.UTF-8',
  'el_GR.UTF-8',
  'hu_HU.UTF-8',
  'ro_RO.UTF-8',
  'C.UTF-8',
];

/** Returns all IANA timezone names; falls back to a small set on older WebViews lacking Intl.supportedValuesOf. */
export function getTimezones(): string[] {
  const intl = Intl as typeof Intl & {
    supportedValuesOf?: (key: string) => string[];
  };
  if (typeof intl.supportedValuesOf === 'function') {
    try {
      return intl.supportedValuesOf('timeZone');
    } catch {
      // Fall through to the static fallback.
    }
  }
  return ['UTC', 'Europe/London', 'Europe/Berlin', 'America/New_York', 'Asia/Tokyo'];
}

type StaticIpConfig = Pick<AutoconfigConfig, 'staticIp' | 'staticMask' | 'staticGateway' | 'staticDns'>;

/** The static IP fields as they are saved and checked: trimmed, blank ones dropped. */
export function trimStaticIp<T extends StaticIpConfig>(c: T): T {
  return {
    ...c,
    staticIp: trimmedOrUndefined(c.staticIp),
    staticMask: trimmedOrUndefined(c.staticMask),
    staticGateway: trimmedOrUndefined(c.staticGateway),
    staticDns: trimmedOrUndefined(c.staticDns),
  };
}

export function staticIpConfigErrors(c: StaticIpConfig): StaticIpErrors {
  const s = trimStaticIp(c);
  return staticIpErrors(s.staticIp, s.staticMask, s.staticGateway, s.staticDns);
}

/** Offset of an IANA time zone right now, as "UTC+2"; empty when the WebView cannot tell. */
export function timezoneOffset(timeZone: string, at: Date = new Date()): string {
  try {
    const part = new Intl.DateTimeFormat('en-US', { timeZone, timeZoneName: 'shortOffset' })
      .formatToParts(at)
      .find((p) => p.type === 'timeZoneName')?.value;
    return part ? part.replace('GMT', 'UTC') : '';
  } catch {
    return '';
  }
}

/** A glibc locale ("it_IT.UTF-8") named in the UI language ("Italian (Italy)"); the raw value otherwise. */
export function localeLabel(locale: string, uiLanguage: string): string {
  const tag = locale.split('.')[0].replace('_', '-');
  try {
    return new Intl.DisplayNames([uiLanguage], { type: 'language' }).of(tag) ?? locale;
  } catch {
    return locale;
  }
}

/** The common locale that matches an app language ("pt-BR" gives pt_BR.UTF-8, "it" gives it_IT.UTF-8). */
export function localeForLanguage(language: string): string {
  const [lang, region] = language.split('-');
  const exact = region && COMMON_LOCALES.find((l) => l.startsWith(`${lang}_${region.toUpperCase()}.`));
  return exact || COMMON_LOCALES.find((l) => l.startsWith(`${lang}_`)) || '';
}

const byteLength = (text: string) => new TextEncoder().encode(text).length;

/** 8 to 63 ASCII characters, or a ready 64-hex key. */
export function isWifiKeyValid(key: string): boolean {
  if (/^[0-9a-fA-F]{64}$/.test(key)) return true;
  return key.length >= AUTOCONFIG.WIFI_KEY_MIN && key.length <= AUTOCONFIG.WIFI_KEY_MAX && /^[\x20-\x7e]*$/.test(key);
}

export const isSsidTooLong = (ssid: string) => byteLength(ssid) > AUTOCONFIG.SSID_MAX_BYTES;

export const isUserScriptTooLarge = (script: string) => byteLength(script) > AUTOCONFIG.USER_SCRIPT_MAX_BYTES;

/** RFC 1123, as `is_hostname` in src-tauri/src/provision/bundle.rs */
export function isValidHostname(name: string): boolean {
  if (name.length > AUTOCONFIG.HOSTNAME_MAX_BYTES) return false;
  return name.split('.').every((label) => label.length <= AUTOCONFIG.HOSTNAME_LABEL_MAX && /^[A-Za-z0-9](?:[A-Za-z0-9-]*[A-Za-z0-9])?$/.test(label));
}

const keyLines = (text: string) => text.split(/\r?\n/).map((line) => line.trim()).filter(Boolean);

// A public key has a type and a base64 body on one line, after options if any; a pasted PEM has neither.
const PUBLIC_KEY_LINE =
  /(?:^|\s)(?:ssh-(?:ed25519|rsa|dss)|ecdsa-sha2-nistp(?:256|384|521)|sk-ssh-ed25519@openssh\.com|sk-ecdsa-sha2-nistp256@openssh\.com)\s+[A-Za-z0-9+/]+={0,3}(?:\s|$)/;

const isPublicKeyLine = (line: string) => PUBLIC_KEY_LINE.test(line);

export const countAuthorizedKeys = (text: string) => keyLines(text).filter(isPublicKeyLine).length;

/** Public keys and comments only. A line starting with [ is also what `check` in src-tauri/src/provision/bundle.rs refuses on the card. */
export function areAuthorizedKeysValid(text: string): boolean {
  return keyLines(text).every(
    // eslint-disable-next-line no-control-regex -- control characters are what this looks for
    (line) => !line.startsWith('[') && !/[\x00-\x1f\x7f]/.test(line) && (line.startsWith('#') || isPublicKeyLine(line))
  );
}

export function mergeAuthorizedKeys(current: string, imported: string): string {
  return [...new Set([...keyLines(current), ...keyLines(imported)])].join('\n');
}

export type ProfileGlyph = 'wifi' | 'cable' | 'user' | 'timezone' | 'keys';

export function profileGlyphs(c: AutoconfigConfig): ProfileGlyph[] {
  const glyphs: ProfileGlyph[] = [];
  if (c.wifiEnabled) glyphs.push('wifi');
  else if (c.ethernetEnabled) glyphs.push('cable');
  if (c.userName?.trim()) glyphs.push('user');
  if (c.timezone || c.locale) glyphs.push('timezone');
  if (c.sshAuthorizedKeys?.trim()) glyphs.push('keys');
  return glyphs;
}

// Twin of provision::is_empty: true when nothing would be applied and the image goes out byte for byte.
export function presetIsEmpty(c: AutoconfigConfig): boolean {
  const set = (v?: string | null) => !!v?.trim();
  const flag = (v?: boolean | null) => v !== undefined && v !== null;
  return !(
    c.wifiEnabled === true ||
    c.useStaticIp === true ||
    set(c.hostname) ||
    set(c.timezone) ||
    set(c.locale) ||
    set(c.keyboardLayout) ||
    set(c.userName) ||
    set(c.userRealName) ||
    set(c.userPasswordHash) ||
    !!c.userShell ||
    set(c.rootPasswordHash) ||
    flag(c.sshEnabled) ||
    flag(c.sshPasswordAuth) ||
    flag(c.sshRootLogin) ||
    set(c.sshAuthorizedKeys) ||
    !!c.overlays?.some(set) ||
    set(c.userScript)
  );
}

/** root keeps the image's password 1234 and the board is on a network or runs SSH, which it does unless the profile turns it off */
export function defaultRootExposed(c: AutoconfigConfig): boolean {
  return !c.userName?.trim() && !c.rootPasswordHash?.trim() && (c.wifiEnabled === true || c.sshEnabled !== false);
}

/** How to reach the board after the first boot; null when SSH is off or nobody could log in over it */
export function sshCommand(c: AutoconfigConfig): string | null {
  const user = c.userName?.trim();
  if (c.sshEnabled === false || (!user && c.sshRootLogin === false)) return null;
  const login = user || 'root';
  const fixed = c.useStaticIp ? c.staticIp?.trim() : '';
  return `ssh ${login}@${fixed || `${c.hostname?.trim() || AUTOCONFIG.HOSTNAME_OF_IMAGE}.local`}`;
}

export function settingsAvailable(support: ImageSettingsSupport | undefined): boolean {
  return support?.kind === 'supported' || support?.kind === 'deferred';
}

/** Why an image takes no settings, or what happens with them; one sentence for each answer of the check */
export function settingsSupportText(support: ImageSettingsSupport, t: TFn): string {
  return t(`flash.settings.${support.kind}`, { reason: support.kind === 'unsupportedFs' ? support.reason : '' });
}

/** Where public SSH keys are imported from: a forge account, or any link. */
export type SshKeySource = 'github' | 'gitlab' | 'link';

/** Labels are brand names, never translated. */
export const SSH_KEY_FORGES = {
  github: { label: 'GitHub', prefix: 'https://github.com/', display: 'github.com/' },
  gitlab: { label: 'GitLab', prefix: 'https://gitlab.com/', display: 'gitlab.com/' },
} as const;

const KEY_USER_PATTERN: Record<keyof typeof SSH_KEY_FORGES, RegExp> = {
  github: /^[A-Za-z0-9](?:[A-Za-z0-9-]{0,38})$/,
  gitlab: /^[A-Za-z0-9_][A-Za-z0-9_.-]{0,254}$/,
};

export function isValidKeyUser(forge: keyof typeof SSH_KEY_FORGES, user: string): boolean {
  return KEY_USER_PATTERN[forge].test(user.trim());
}

export function keyLookupSource(source: SshKeySource): KeyLookupSource {
  return source === 'link' ? 'url' : source;
}

/** ISO 3166-1 alpha-2 country code paired with an English display name. */
export interface CountryOption {
  code: string;
  name: string;
}

/** ISO 3166-1 alpha-2 country codes for the Wi-Fi regulatory domain selector. */
export const WIFI_COUNTRY_CODES: readonly CountryOption[] = [
  { code: 'AD', name: 'Andorra' },
  { code: 'AE', name: 'United Arab Emirates' },
  { code: 'AF', name: 'Afghanistan' },
  { code: 'AG', name: 'Antigua and Barbuda' },
  { code: 'AI', name: 'Anguilla' },
  { code: 'AL', name: 'Albania' },
  { code: 'AM', name: 'Armenia' },
  { code: 'AN', name: 'Netherlands Antilles' },
  { code: 'AO', name: 'Angola' },
  { code: 'AR', name: 'Argentina' },
  { code: 'AS', name: 'American Samoa' },
  { code: 'AT', name: 'Austria' },
  { code: 'AU', name: 'Australia' },
  { code: 'AW', name: 'Aruba' },
  { code: 'AZ', name: 'Azerbaijan' },
  { code: 'BA', name: 'Bosnia and Herzegovina' },
  { code: 'BB', name: 'Barbados' },
  { code: 'BD', name: 'Bangladesh' },
  { code: 'BE', name: 'Belgium' },
  { code: 'BF', name: 'Burkina Faso' },
  { code: 'BG', name: 'Bulgaria' },
  { code: 'BH', name: 'Bahrain' },
  { code: 'BI', name: 'Burundi' },
  { code: 'BJ', name: 'Benin' },
  { code: 'BL', name: 'Saint Barthelemy' },
  { code: 'BM', name: 'Bermuda' },
  { code: 'BN', name: 'Brunei Darussalam' },
  { code: 'BO', name: 'Bolivia' },
  { code: 'BR', name: 'Brazil' },
  { code: 'BS', name: 'Bahamas' },
  { code: 'BT', name: 'Bhutan' },
  { code: 'BW', name: 'Botswana' },
  { code: 'BY', name: 'Belarus' },
  { code: 'BZ', name: 'Belize' },
  { code: 'CA', name: 'Canada' },
  { code: 'CD', name: 'Congo (Kinshasa)' },
  { code: 'CF', name: 'Central African Republic' },
  { code: 'CG', name: 'Congo (Brazzaville)' },
  { code: 'CH', name: 'Switzerland' },
  { code: 'CI', name: "Cote d'Ivoire" },
  { code: 'CL', name: 'Chile' },
  { code: 'CM', name: 'Cameroon' },
  { code: 'CN', name: 'China' },
  { code: 'CO', name: 'Colombia' },
  { code: 'CR', name: 'Costa Rica' },
  { code: 'CU', name: 'Cuba' },
  { code: 'CV', name: 'Cabo Verde' },
  { code: 'CX', name: 'Christmas Island' },
  { code: 'CY', name: 'Cyprus' },
  { code: 'CZ', name: 'Czechia' },
  { code: 'DE', name: 'Germany' },
  { code: 'DJ', name: 'Djibouti' },
  { code: 'DK', name: 'Denmark' },
  { code: 'DM', name: 'Dominica' },
  { code: 'DO', name: 'Dominican Republic' },
  { code: 'DZ', name: 'Algeria' },
  { code: 'EC', name: 'Ecuador' },
  { code: 'EE', name: 'Estonia' },
  { code: 'EG', name: 'Egypt' },
  { code: 'ER', name: 'Eritrea' },
  { code: 'ES', name: 'Spain' },
  { code: 'ET', name: 'Ethiopia' },
  { code: 'FI', name: 'Finland' },
  { code: 'FJ', name: 'Fiji' },
  { code: 'FM', name: 'Micronesia' },
  { code: 'FR', name: 'France' },
  { code: 'GA', name: 'Gabon' },
  { code: 'GB', name: 'United Kingdom' },
  { code: 'GD', name: 'Grenada' },
  { code: 'GE', name: 'Georgia' },
  { code: 'GF', name: 'French Guiana' },
  { code: 'GH', name: 'Ghana' },
  { code: 'GL', name: 'Greenland' },
  { code: 'GM', name: 'Gambia' },
  { code: 'GN', name: 'Guinea' },
  { code: 'GP', name: 'Guadeloupe' },
  { code: 'GQ', name: 'Equatorial Guinea' },
  { code: 'GR', name: 'Greece' },
  { code: 'GT', name: 'Guatemala' },
  { code: 'GU', name: 'Guam' },
  { code: 'GW', name: 'Guinea-Bissau' },
  { code: 'GY', name: 'Guyana' },
  { code: 'HK', name: 'Hong Kong' },
  { code: 'HN', name: 'Honduras' },
  { code: 'HR', name: 'Croatia' },
  { code: 'HT', name: 'Haiti' },
  { code: 'HU', name: 'Hungary' },
  { code: 'ID', name: 'Indonesia' },
  { code: 'IE', name: 'Ireland' },
  { code: 'IL', name: 'Israel' },
  { code: 'IN', name: 'India' },
  { code: 'IQ', name: 'Iraq' },
  { code: 'IR', name: 'Iran' },
  { code: 'IS', name: 'Iceland' },
  { code: 'IT', name: 'Italy' },
  { code: 'JM', name: 'Jamaica' },
  { code: 'JO', name: 'Jordan' },
  { code: 'JP', name: 'Japan' },
  { code: 'KE', name: 'Kenya' },
  { code: 'KG', name: 'Kyrgyzstan' },
  { code: 'KH', name: 'Cambodia' },
  { code: 'KI', name: 'Kiribati' },
  { code: 'KM', name: 'Comoros' },
  { code: 'KN', name: 'Saint Kitts and Nevis' },
  { code: 'KP', name: 'North Korea' },
  { code: 'KR', name: 'South Korea' },
  { code: 'KW', name: 'Kuwait' },
  { code: 'KY', name: 'Cayman Islands' },
  { code: 'KZ', name: 'Kazakhstan' },
  { code: 'LA', name: 'Laos' },
  { code: 'LB', name: 'Lebanon' },
  { code: 'LC', name: 'Saint Lucia' },
  { code: 'LI', name: 'Liechtenstein' },
  { code: 'LK', name: 'Sri Lanka' },
  { code: 'LR', name: 'Liberia' },
  { code: 'LS', name: 'Lesotho' },
  { code: 'LT', name: 'Lithuania' },
  { code: 'LU', name: 'Luxembourg' },
  { code: 'LV', name: 'Latvia' },
  { code: 'LY', name: 'Libya' },
  { code: 'MA', name: 'Morocco' },
  { code: 'MC', name: 'Monaco' },
  { code: 'MD', name: 'Moldova' },
  { code: 'ME', name: 'Montenegro' },
  { code: 'MF', name: 'Saint Martin' },
  { code: 'MG', name: 'Madagascar' },
  { code: 'MH', name: 'Marshall Islands' },
  { code: 'MK', name: 'North Macedonia' },
  { code: 'ML', name: 'Mali' },
  { code: 'MM', name: 'Myanmar' },
  { code: 'MN', name: 'Mongolia' },
  { code: 'MO', name: 'Macao' },
  { code: 'MP', name: 'Northern Mariana Islands' },
  { code: 'MQ', name: 'Martinique' },
  { code: 'MR', name: 'Mauritania' },
  { code: 'MT', name: 'Malta' },
  { code: 'MU', name: 'Mauritius' },
  { code: 'MV', name: 'Maldives' },
  { code: 'MW', name: 'Malawi' },
  { code: 'MX', name: 'Mexico' },
  { code: 'MY', name: 'Malaysia' },
  { code: 'MZ', name: 'Mozambique' },
  { code: 'NA', name: 'Namibia' },
  { code: 'NE', name: 'Niger' },
  { code: 'NG', name: 'Nigeria' },
  { code: 'NI', name: 'Nicaragua' },
  { code: 'NL', name: 'Netherlands' },
  { code: 'NO', name: 'Norway' },
  { code: 'NP', name: 'Nepal' },
  { code: 'NR', name: 'Nauru' },
  { code: 'NZ', name: 'New Zealand' },
  { code: 'OM', name: 'Oman' },
  { code: 'PA', name: 'Panama' },
  { code: 'PE', name: 'Peru' },
  { code: 'PF', name: 'French Polynesia' },
  { code: 'PG', name: 'Papua New Guinea' },
  { code: 'PH', name: 'Philippines' },
  { code: 'PK', name: 'Pakistan' },
  { code: 'PL', name: 'Poland' },
  { code: 'PM', name: 'Saint Pierre and Miquelon' },
  { code: 'PR', name: 'Puerto Rico' },
  { code: 'PT', name: 'Portugal' },
  { code: 'PW', name: 'Palau' },
  { code: 'PY', name: 'Paraguay' },
  { code: 'QA', name: 'Qatar' },
  { code: 'RE', name: 'Reunion' },
  { code: 'RO', name: 'Romania' },
  { code: 'RS', name: 'Serbia' },
  { code: 'RU', name: 'Russia' },
  { code: 'RW', name: 'Rwanda' },
  { code: 'SA', name: 'Saudi Arabia' },
  { code: 'SB', name: 'Solomon Islands' },
  { code: 'SC', name: 'Seychelles' },
  { code: 'SD', name: 'Sudan' },
  { code: 'SE', name: 'Sweden' },
  { code: 'SG', name: 'Singapore' },
  { code: 'SI', name: 'Slovenia' },
  { code: 'SK', name: 'Slovakia' },
  { code: 'SL', name: 'Sierra Leone' },
  { code: 'SM', name: 'San Marino' },
  { code: 'SN', name: 'Senegal' },
  { code: 'SO', name: 'Somalia' },
  { code: 'SR', name: 'Suriname' },
  { code: 'SS', name: 'South Sudan' },
  { code: 'ST', name: 'Sao Tome and Principe' },
  { code: 'SV', name: 'El Salvador' },
  { code: 'SY', name: 'Syria' },
  { code: 'SZ', name: 'Eswatini' },
  { code: 'TC', name: 'Turks and Caicos Islands' },
  { code: 'TD', name: 'Chad' },
  { code: 'TG', name: 'Togo' },
  { code: 'TH', name: 'Thailand' },
  { code: 'TJ', name: 'Tajikistan' },
  { code: 'TL', name: 'Timor-Leste' },
  { code: 'TM', name: 'Turkmenistan' },
  { code: 'TN', name: 'Tunisia' },
  { code: 'TO', name: 'Tonga' },
  { code: 'TR', name: 'Turkiye' },
  { code: 'TT', name: 'Trinidad and Tobago' },
  { code: 'TV', name: 'Tuvalu' },
  { code: 'TW', name: 'Taiwan' },
  { code: 'TZ', name: 'Tanzania' },
  { code: 'UA', name: 'Ukraine' },
  { code: 'UG', name: 'Uganda' },
  { code: 'US', name: 'United States' },
  { code: 'UY', name: 'Uruguay' },
  { code: 'UZ', name: 'Uzbekistan' },
  { code: 'VC', name: 'Saint Vincent and the Grenadines' },
  { code: 'VE', name: 'Venezuela' },
  { code: 'VI', name: 'Virgin Islands (U.S.)' },
  { code: 'VN', name: 'Vietnam' },
  { code: 'VU', name: 'Vanuatu' },
  { code: 'WF', name: 'Wallis and Futuna' },
  { code: 'WS', name: 'Samoa' },
  { code: 'YE', name: 'Yemen' },
  { code: 'YT', name: 'Mayotte' },
  { code: 'ZA', name: 'South Africa' },
  { code: 'ZM', name: 'Zambia' },
  { code: 'ZW', name: 'Zimbabwe' },
];

/** An empty list means the profile is offered for every board */
export function profileBoards(p: AutoconfigProfile): ProfileBoard[] {
  return p.boards ?? [];
}

/** A null board (custom image with no known board) only gets the all-board profiles */
export function isProfileOfferedFor(p: AutoconfigProfile, boardSlug: string | null): boolean {
  const boards = profileBoards(p);
  if (boards.length === 0) return true;
  return boardSlug !== null && boards.some((b) => b.slug === boardSlug);
}
