// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2026 Daniele Briguglio, superkali@armbian.com

import { useId } from 'react';
import { useTranslation } from 'react-i18next';
import { RotateCcw } from 'lucide-react';
import { DevSegmented } from './DevControls';
import { useRestoreSavedAppearance } from '../../hooks/useRestoreSavedAppearance';
import { useTheme } from '../../contexts/ThemeContext';
import { useMotion } from '../../contexts/MotionContext';
import { useToasts } from '../../hooks/useToasts';
import {
  DEV_MOTION_OPTIONS,
  DEV_PREVIEW_LANGUAGES,
  DEV_SCENARIOS,
  DEV_THEME_OPTIONS,
} from '../../config/devScenarios';
import { getErrorMessage } from '../../utils';

const { ICON_SIZE } = DEV_SCENARIOS;

export function DevApp() {
  const languageId = useId();
  const { i18n } = useTranslation();
  const { theme, setTheme } = useTheme();
  const { motion, setMotion } = useMotion();
  const { showSuccess, showError } = useToasts();
  const restoreSavedAppearance = useRestoreSavedAppearance();

  const restoreSaved = async () => {
    try {
      await restoreSavedAppearance();
      showSuccess('Saved appearance restored');
    } catch (err) {
      showError(getErrorMessage(err, 'Could not read the saved appearance'));
    }
  };

  return (
    <div className="dev-stack">
      <div className="dev-grid dev-grid--fit">
        <DevSegmented
          label="Theme"
          options={DEV_THEME_OPTIONS}
          value={theme}
          onChange={(next) => setTheme(next, { persist: false })}
        />
        <DevSegmented
          label="Motion"
          options={DEV_MOTION_OPTIONS}
          value={motion}
          onChange={(next) => setMotion(next, { persist: false })}
        />
      </div>
      <div className="dev-field">
        <label className="dev-field__label" htmlFor={languageId}>
          Language
        </label>
        <select
          id={languageId}
          className="settings-select dev-select"
          value={i18n.resolvedLanguage ?? i18n.language}
          onChange={(e) => i18n.changeLanguage(e.target.value)}
        >
          {DEV_PREVIEW_LANGUAGES.map((l) => (
            <option key={l.code} value={l.code}>
              {l.name} ({l.code})
            </option>
          ))}
        </select>
      </div>
      <p className="dev-note">Previews are not saved. Settings keeps your saved choice.</p>
      <button type="button" className="dev-btn dev-btn--quiet dev-btn--block" onClick={restoreSaved}>
        <RotateCcw size={ICON_SIZE.SM} aria-hidden="true" />
        Restore saved appearance
      </button>
    </div>
  );
}
