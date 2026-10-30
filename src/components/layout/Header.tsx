// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2025-2026 Daniele Briguglio, superkali@armbian.com

import type { Ref } from 'react';
import { ArrowLeft, Check } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { APP_ICON } from '../../assets';
import type { ImageInfo, BlockDevice, BoardInfo, Manufacturer, SelectionStep } from '../../types';
import { isEdlImage } from '../../types';
import { SettingsButton } from '../settings';

interface HeaderProps {
  selectedImage?: ImageInfo | null;
  selectedDevice?: BlockDevice | null;
  selectedManufacturer?: Manufacturer | null;
  selectedBoard?: BoardInfo | null;
  /** The catalog offers several manufacturers, so the manufacturer step exists */
  showManufacturerStep?: boolean;
  /** The catalog offers several boards, so the board step exists */
  showBoardStep?: boolean;
  onReset?: () => void;
  onNavigateToStep?: (step: SelectionStep) => void;
  isFlashing?: boolean;
  /** Hide the step progress pill (e.g. on the welcome landing). */
  hideSteps?: boolean;
  /** Hide the settings gear (e.g. on the welcome landing). */
  hideSettings?: boolean;
  /** Hide the wordmark (e.g. on the welcome landing, where the hero already brands it). */
  hideLogo?: boolean;
  /** True briefly after leaving the welcome screen to drive the one-shot entrance animation. */
  entering?: boolean;
  /** The settings page shows: the step capsule gives way to the "Back to flashing" pill. */
  settingsOpen?: boolean;
  onOpenSettings?: () => void;
  onCloseSettings?: () => void;
  settingsButtonRef?: Ref<HTMLButtonElement>;
}

export function Header({
  selectedImage,
  selectedDevice,
  selectedManufacturer = null,
  selectedBoard = null,
  showManufacturerStep = false,
  showBoardStep = false,
  onReset,
  onNavigateToStep,
  isFlashing,
  hideSteps = false,
  hideSettings = false,
  hideLogo = false,
  entering = false,
  settingsOpen = false,
  onOpenSettings,
  onCloseSettings,
  settingsButtonRef,
}: HeaderProps) {
  const { t } = useTranslation();
  // EDL targets are USB devices in download mode, not storage drives.
  const isEdl = !!selectedImage && isEdlImage(selectedImage);
  const targetLabel = t(isEdl ? 'header.stepDevice' : 'header.stepStorage');
  // Manufacturer and board steps exist only when the catalog offers a choice.
  const steps = [
    ...(showManufacturerStep
      ? [{ key: 'manufacturer' as SelectionStep, label: t('header.stepManufacturer'), completed: !!selectedManufacturer }]
      : []),
    ...(showBoardStep
      ? [{ key: 'board' as SelectionStep, label: t('header.stepBoard'), completed: !!selectedBoard }]
      : []),
    { key: 'image' as SelectionStep, label: t('header.stepImage'), completed: !!selectedImage },
    { key: 'device' as SelectionStep, label: targetLabel, completed: !!selectedDevice },
  ];

  const canReset = !isFlashing && !settingsOpen && !!onReset;

  function handleLogoClick() {
    if (canReset) onReset?.();
  }

  const canNavigateSteps = !isFlashing && !settingsOpen && !!onNavigateToStep;
  const showSteps = !hideSteps;

  function handleStepClick(step: SelectionStep, completed: boolean) {
    if (canNavigateSteps && completed) {
      onNavigateToStep!(step);
    }
  }

  return (
    <header className={`header ${entering ? 'is-entering' : ''}`} data-tauri-drag-region>
      {/* Wordmark hidden on the welcome landing, where the hero already brands the app. */}
      {hideLogo ? (
        <div className="header-left" />
      ) : (
        <div
          className={`header-left ${canReset ? 'clickable' : ''}`}
          onClick={handleLogoClick}
          title={canReset ? t('header.resetTooltip') : undefined}
        >
          <img src={APP_ICON} alt="" aria-hidden="true" className="logo-main" />
          <span className="logo-name">Wukong Imager</span>
        </div>
      )}
      <div className="header-right">
        <div className={`header-controls${settingsOpen ? ' is-settings' : ''}`}>
          {showSteps && (
            <div className="header-steps" aria-hidden={settingsOpen || undefined} inert={settingsOpen || undefined}>
              {steps.map((step, index) => (
                <div
                  key={step.key}
                  className={`header-step ${step.completed ? 'completed' : ''} ${canNavigateSteps && step.completed ? 'clickable' : ''}`}
                  onClick={() => handleStepClick(step.key, step.completed)}
                  title={canNavigateSteps && step.completed ? t('header.stepTooltip', { step: step.label }) : undefined}
                >
                  <span className="header-step-indicator">
                    {step.completed ? <Check size={14} /> : (index + 1)}
                  </span>
                  <span className="header-step-label">{step.label}</span>
                </div>
              ))}
            </div>
          )}
          {!hideSettings && (
            <button
              type="button"
              className="header-back"
              onClick={onCloseSettings}
              aria-hidden={!settingsOpen || undefined}
              inert={!settingsOpen || undefined}
            >
              <ArrowLeft size={16} aria-hidden="true" />
              {t('settings.backToFlashing')}
            </button>
          )}
        </div>
        {!hideSettings && (
          <SettingsButton
            ref={settingsButtonRef}
            active={settingsOpen}
            onClick={() => (settingsOpen ? onCloseSettings?.() : onOpenSettings?.())}
          />
        )}
      </div>
    </header>
  );
}
