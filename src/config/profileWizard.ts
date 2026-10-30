// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2026 Daniele Briguglio, superkali@armbian.com

import type { AutoconfigProfile, ProfileBoard } from '../types';
import type { TFn } from '../utils/errorUtils';
import {
  accessError,
  editorIpErrors,
  fromEditorModel,
  hasWifi,
  hostnameError,
  profileFacts,
  scriptError,
  ssidError,
  toEditorForm,
  userError,
  wifiKeyError,
  type EditorForm,
  type EditorModel,
  type ProfileFact,
} from './profileEditorModel';

export const WIZARD_STEPS = ['name', 'network', 'user', 'locale', 'access', 'device', 'review'] as const;
export type WizardStep = (typeof WIZARD_STEPS)[number];

/** Steps the left column lists with a status, and the only ones that can be skipped */
export const WIZARD_ITEMS = ['network', 'user', 'locale', 'access', 'device'] as const satisfies readonly WizardStep[];
export type WizardItem = (typeof WIZARD_ITEMS)[number];
export type WizardSkips = Partial<Record<WizardItem, boolean>>;

/** Columns of the Name step boards row: four boards per page */
export const WIZARD_BOARD_SLOTS = 4;

export const isWizardItem = (step: WizardStep): step is WizardItem => (WIZARD_ITEMS as readonly string[]).includes(step);

// Index of each item in the list profileFacts() returns.
const FACT_INDEX: Record<WizardItem, number> = { network: 0, user: 1, access: 2, locale: 3, device: 4 };

export const wizardItemFact = (item: WizardItem, facts: readonly ProfileFact[]): ProfileFact => facts[FACT_INDEX[item]];

/** What an untouched or skipped item keeps, worded like the choice its step opens on */
export const WIZARD_DEFAULT_KEY: Record<WizardItem, string> = {
  network: 'settings.autoconfig.armbianDefaults',
  user: 'settings.autoconfig.wizard.user.later',
  locale: 'settings.autoconfig.armbianDefaults',
  access: 'settings.autoconfig.armbianDefaults',
  device: 'settings.autoconfig.armbianDefaults',
};

/** Each step opens on what Next would save untouched; a known board scopes it. */
export function emptyWizardModel(defaults: { timezone: string; locale: string }, board: ProfileBoard | null): EditorForm {
  return { ...toEditorForm(null, defaults), scope: board ? 'some' : 'all', boards: board ? [board] : [] };
}

const underAutoconfig = (key: string | null) => (key ? `settings.autoconfig.${key}` : null);

/** The form the access step answers for: a skipped user step creates nobody */
export const wizardAccessForm = (form: EditorForm, skipped: WizardSkips): EditorForm =>
  skipped.user ? { ...form, user: { ...form.user, mode: 'later' } } : form;

/** i18n key of what stops Next on a step, or null; the static IP checks show on their fields */
export function wizardStepError(step: WizardStep, form: EditorForm, skipped: WizardSkips = {}): string | null {
  switch (step) {
    case 'name':
      if (!form.name.trim()) return 'settings.autoconfig.nameRequired';
      return form.scope === 'some' && !form.boards.length ? 'settings.autoconfig.editor.pickBoard' : null;
    case 'network': {
      const n = form.network;
      if (!n.apply) return null;
      if (hasWifi(n.mode)) {
        const error = ssidError(n) ?? wifiKeyError(form) ?? (n.country.trim() ? null : 'editor.countryRequired');
        if (error) return underAutoconfig(error);
      }
      return n.ip === 'fixed' && !n.address.trim() ? 'settings.autoconfig.wizard.addressRequired' : null;
    }
    case 'user':
      return underAutoconfig(userError(form));
    case 'access':
      return underAutoconfig(accessError(wizardAccessForm(form, skipped)));
    case 'device':
      return underAutoconfig(hostnameError(form.device) ?? scriptError(form.device));
    default:
      return null;
  }
}

export const wizardStepBlocked = (step: WizardStep, form: EditorForm, skipped: WizardSkips = {}): boolean =>
  !!wizardStepError(step, form, skipped) || (step === 'network' && form.network.apply && Object.keys(editorIpErrors(form.network)).length > 0);

/** A skipped step writes nothing, so the image keeps its own defaults there. */
export function wizardModel(form: EditorForm, skipped: WizardSkips): EditorForm {
  const { secrets } = form;
  return {
    ...form,
    network: skipped.network ? { ...form.network, apply: false } : form.network,
    user: skipped.user ? { ...form.user, mode: 'later' } : form.user,
    region: skipped.locale ? { ...form.region, apply: false } : form.region,
    access: skipped.access ? { ...form.access, apply: false, rootPasswordHash: '' } : form.access,
    device: skipped.device ? { hostname: '', overlays: [], script: '' } : form.device,
    secrets: {
      wifiKey: skipped.network ? '' : secrets.wifiKey,
      userPassword: skipped.user ? '' : secrets.userPassword,
      rootPassword: skipped.access ? '' : secrets.rootPassword,
    },
  };
}

/** i18n key of what stops the profile as a whole: going back to an earlier step can undo what the access step checked */
export function wizardProfileError(form: EditorForm, skipped: WizardSkips): string | null {
  const model = wizardModel(form, skipped);
  return underAutoconfig(accessError(model) ?? (model.network.apply ? editorIpErrors(model.network).mask : null) ?? null);
}

/** Takes a sealed model: what reaches the profile has no plain text to carry. */
export const wizardProfile = (m: EditorModel): Pick<AutoconfigProfile, 'name' | 'config' | 'boards'> => fromEditorModel(m);

/** profileFacts of what the wizard saves; a skipped area reads as the image default it keeps */
export function wizardFacts(form: EditorForm, skipped: WizardSkips, t: TFn): ProfileFact[] {
  const facts = profileFacts(wizardModel(form, skipped), t);
  return facts.map((fact, index) => {
    const item = WIZARD_ITEMS.find((i) => FACT_INDEX[i] === index);
    return item && skipped[item] ? { ...fact, text: t(WIZARD_DEFAULT_KEY[item]), tone: 'plain' } : fact;
  });
}
