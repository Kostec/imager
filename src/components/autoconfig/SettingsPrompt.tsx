// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2026 Daniele Briguglio, superkali@armbian.com

import { useEffect, useRef } from 'react';
import { createPortal } from 'react-dom';
import { SlidersHorizontal } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { AutoconfigProfile } from '../../types';
import { ProfileGlyphs } from './ProfileGlyphs';

interface SettingsPromptProps {
  profiles: readonly AutoconfigProfile[];
  /** A saved profile goes into the summary; writing waits for the next press, so the summary shows it first */
  onPick: (id: string) => void;
  onNewProfile: () => void;
  /** Write the image as it is */
  onSkip: () => void;
  /** Back to the summary, nothing written */
  onClose: () => void;
}

/** Asked when the image takes settings and no profile is picked: the summary row alone was easy to miss. */
export function SettingsPrompt({ profiles, onPick, onNewProfile, onSkip, onClose }: SettingsPromptProps) {
  const { t } = useTranslation();
  const setUpRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    setUpRef.current?.focus();
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [onClose]);

  // A click beside the dialog only closes it: writing without settings takes the explicit button.
  return createPortal(
    <div className="modal-overlay" onClick={onClose}>
      <div
        className="modal confirm-modal settings-prompt"
        role="dialog"
        aria-modal="true"
        aria-labelledby="settings-prompt-title"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="confirm-icon">
          <SlidersHorizontal size={28} />
        </div>
        <h3 id="settings-prompt-title" className="confirm-title">{t('flash.settingsPrompt.title')}</h3>
        <p className="confirm-text">{t('flash.settingsPrompt.text')}</p>
        {profiles.length > 0 && (
          <>
            <p className="settings-prompt__hint">{t('flash.settingsPrompt.saved')}</p>
            <div className="settings-prompt__list">
              {profiles.map((profile) => (
                <button
                  key={profile.id}
                  type="button"
                  className="profile-menu__item settings-prompt__item"
                  onClick={() => onPick(profile.id)}
                >
                  <span className="profile-menu__name">{profile.name}</span>
                  <ProfileGlyphs config={profile.config} />
                </button>
              ))}
            </div>
          </>
        )}
        <div className="confirm-actions">
          <button type="button" className="btn btn-secondary" onClick={onSkip}>
            {t('flash.settingsPrompt.skip')}
          </button>
          <button ref={setUpRef} type="button" className="btn btn-primary" onClick={onNewProfile}>
            {t('flash.settingsPrompt.setUp')}
          </button>
        </div>
      </div>
    </div>,
    document.body
  );
}
