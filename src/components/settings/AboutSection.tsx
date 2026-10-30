// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2025-2026 Daniele Briguglio, superkali@armbian.com

import { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { getVersion } from '@tauri-apps/api/app';
import { Bug, ExternalLink, FileText, GitFork, Github, Scale, type LucideIcon } from 'lucide-react';
import { getTauriVersion, getSystemInfo, openExternal } from '../../hooks/useTauri';
import { getDeveloperMode, setDeveloperMode } from '../../hooks/useSettings';
import { useAsyncData } from '../../hooks/useAsyncData';
import { useSettingToggle } from '../../hooks/useSettingToggle';
import { ErrorDisplay } from '../shared';
import { SettingsGroup, SettingsRow, ToggleRow } from './controls';
import { LogsModal } from './LogsModal';
import { LINKS, PLATFORM_LABEL, SETTINGS } from '../../config/constants';
import { displayUrl } from '../../utils';
import appIcon from '../../../src-tauri/icons/icon.png';

interface AboutLink {
  icon: LucideIcon;
  labelKey: string;
  url: string;
  subKey?: string;
}

const ABOUT_LINKS: readonly AboutLink[] = [
  { icon: Github, labelKey: 'settings.githubRepo', url: LINKS.GITHUB_REPO },
  { icon: Bug, labelKey: 'settings.reportIssue', url: LINKS.ISSUES, subKey: 'settings.about.reportSub' },
  { icon: GitFork, labelKey: 'settings.about.upstream', url: LINKS.UPSTREAM },
  { icon: Scale, labelKey: 'settings.about.license', url: LINKS.LICENSE, subKey: 'settings.about.licenseSub' },
];

export function AboutSection() {
  const { t } = useTranslation();
  const [logsOpen, setLogsOpen] = useState(false);
  const developerMode = useSettingToggle(getDeveloperMode, setDeveloperMode, { fallback: SETTINGS.DEFAULTS.DEVELOPER_MODE });
  const titleId = useId();
  const linksId = useId();

  const { data: appInfo, error, reload } = useAsyncData(async () => {
    const [appVersion, tauriVersion, systemInfo] = await Promise.all([getVersion(), getTauriVersion(), getSystemInfo()]);
    return {
      appVersion,
      tauriVersion,
      platform: PLATFORM_LABEL[systemInfo.platform] || systemInfo.platform,
      arch: systemInfo.arch,
    };
  }, []);

  return (
    <div className="about-page">
      <div className="about-page__head">
        <section className="about-page__hero" aria-labelledby={titleId}>
          <span className="about-page__icon">
            <img src={appIcon} alt="" />
          </span>
          <div className="about-page__hero-text">
            <h1 id={titleId} className="about-page__title">Wukong Imager</h1>
            {appInfo && (
              <p className="about-page__meta">
                <span className="about-page__version">v{appInfo.appVersion}</span>
                <span>
                  {appInfo.platform} · {appInfo.arch}
                </span>
              </p>
            )}
          </div>
        </section>
      </div>

      {error && (
        <div className="about-page__error">
          <ErrorDisplay error={error} onRetry={reload} compact />
        </div>
      )}

      <section className="set-group" aria-labelledby={linksId}>
        <h3 id={linksId} className="set-group__eyebrow">
          {t('settings.links')}
        </h3>
        <div className="about-page__links">
          {ABOUT_LINKS.map(({ icon: Icon, labelKey, url, subKey }) => (
            <button key={url} type="button" className="about-link" onClick={() => openExternal(url)}>
              <span className="about-link__icon" aria-hidden="true">
                <Icon size={18} />
              </span>
              <span className="about-link__text">
                <b>{t(labelKey)}</b>
                <small>{subKey ? t(subKey) : displayUrl(url)}</small>
              </span>
              <ExternalLink className="about-link__arrow" size={16} aria-hidden="true" />
            </button>
          ))}
        </div>
      </section>

      <SettingsGroup eyebrow={t('settings.about.logsEyebrow')}>
        <SettingsRow
          label={t('settings.about.sessionLog')}
          description={t('settings.about.sessionLogDesc')}
          control={
            <button type="button" className="btn btn-secondary btn-pill" onClick={() => setLogsOpen(true)}>
              <FileText size={16} aria-hidden="true" />
              {t('settings.about.open')}
            </button>
          }
        />
      </SettingsGroup>

      {developerMode.value !== undefined && (
        <SettingsGroup eyebrow={t('settings.developer')}>
          <ToggleRow
            label={t('settings.developerMode')}
            description={t('settings.developerModeDescription')}
            checked={developerMode.value}
            onChange={(next) => developerMode.set(next)}
            busy={developerMode.busy}
          />
        </SettingsGroup>
      )}

      {appInfo && <p className="about-page__credits">{t('settings.about.credits', { tauri: appInfo.tauriVersion })}</p>}

      <LogsModal isOpen={logsOpen} onClose={() => setLogsOpen(false)} />
    </div>
  );
}
