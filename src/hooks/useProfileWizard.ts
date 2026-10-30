// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2026 Daniele Briguglio, superkali@armbian.com

import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { keyLookupSource, localeForLanguage } from '../config/autoconfig';
import { editorIpErrors, isFormEmpty, keySourceFormatError, sealEditorForm, type EditorForm, type MoreKey } from '../config/profileEditorModel';
import {
  WIZARD_STEPS, emptyWizardModel, isWizardItem, wizardFacts, wizardModel, wizardProfile, wizardStepBlocked, wizardStepError,
  type WizardSkips,
} from '../config/profileWizard';
import type { AutoconfigProfile, ProfileBoard } from '../types';
import { upsertAutoconfigProfile } from './useSettings';
import { useSshKeyCheck } from './useSshKeyCheck';
import { deriveWifiPsk, hashPassword, logWarn } from './useTauri';
import { useToasts } from './useToasts';

type ModelPart = 'network' | 'user' | 'region' | 'access' | 'device' | 'secrets';

export function useProfileWizard(board: ProfileBoard | null, onSaved: (profile: AutoconfigProfile) => void) {
  const { t, i18n } = useTranslation();
  const { showSuccess, showError } = useToasts();

  const [model, setModel] = useState<EditorForm>(() =>
    emptyWizardModel(
      { timezone: Intl.DateTimeFormat().resolvedOptions().timeZone ?? '', locale: localeForLanguage(i18n.language) },
      board
    )
  );
  const [skipped, setSkipped] = useState<WizardSkips>({});
  const [stepIndex, setStepIndex] = useState(0);
  const [furthest, setFurthest] = useState(0);
  const [more, setMore] = useState<Partial<Record<MoreKey, boolean>>>({});
  const [attempted, setAttempted] = useState(false);
  const [saving, setSaving] = useState(false);

  const step = WIZARD_STEPS[stepIndex];
  const stepError = wizardStepError(step, model);
  const blocked = wizardStepBlocked(step, model);
  // Every area left at the image's defaults: nothing to write, so it cannot be created
  const empty = useMemo(() => isFormEmpty(wizardModel(model, skipped)), [model, skipped]);
  const ipErrors = editorIpErrors(model.network);

  const access = model.access;
  const keyCheck = useSshKeyCheck(
    keyLookupSource(access.source),
    access.source === 'link' ? access.keyLink : access.keyUser,
    access.apply && access.enabled && keySourceFormatError(access) === null
  );
  const facts = useMemo(() => wizardFacts(model, skipped, t), [model, skipped, t]);

  const set = (value: Partial<EditorForm>) => setModel((m) => ({ ...m, ...value }));
  const patch = <K extends ModelPart>(key: K, value: Partial<EditorForm[K]>) =>
    setModel((m) => ({ ...m, [key]: { ...m[key], ...value } }));
  const toggleMore = (key: MoreKey) => setMore((m) => ({ ...m, [key]: !m[key] }));

  function go(index: number) {
    setAttempted(false);
    setStepIndex(index);
    setFurthest((f) => Math.max(f, index));
  }

  function next() {
    if (blocked) {
      setAttempted(true);
      // The country and the Fixed IP fields sit under "more": open it so the reason Next refused is in view.
      if (step === 'network') setMore((m) => ({ ...m, network: true }));
      return;
    }
    if (isWizardItem(step)) setSkipped((s) => ({ ...s, [step]: false }));
    go(stepIndex + 1);
  }

  function skip() {
    if (isWizardItem(step)) setSkipped((s) => ({ ...s, [step]: true }));
    go(stepIndex + 1);
  }

  async function create() {
    if (!model.name.trim() || saving || empty) return;
    setSaving(true);
    try {
      const sealed = await sealEditorForm(wizardModel(model, skipped), { hashPassword, deriveWifiPsk });
      const profile: AutoconfigProfile = { id: crypto.randomUUID(), updatedAt: Date.now(), ...wizardProfile(sealed) };
      await upsertAutoconfigProfile(profile);
      showSuccess(t('settings.autoconfig.toastCreated'));
      onSaved(profile);
    } catch (error) {
      logWarn('autoconfig', `Failed to save the profile from the guided wizard: ${error}`);
      showError(t('settings.autoconfig.toastError'));
      setSaving(false);
    }
  }

  const shownError = attempted && stepError ? t(stepError) : null;

  return {
    model, skipped, step, stepIndex, furthest, more, attempted, saving, blocked, empty, stepError, shownError, ipErrors,
    keyCheck, facts, set, patch, toggleMore, go, next, skip, create,
  };
}

export type ProfileWizardState = ReturnType<typeof useProfileWizard>;
