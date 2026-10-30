// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2025-2026 Daniele Briguglio, superkali@armbian.com

import { useTranslation } from 'react-i18next';
import { ArrowRight, Download, HardDrive, ShieldCheck } from 'lucide-react';
import { SETTINGS } from '../../config';
import {
  getAllowSystemDevices,
  setAllowSystemDevices,
  getSkipVerify,
  setSkipVerify,
} from '../../hooks/useSettings';
import { useSettingToggle } from '../../hooks/useSettingToggle';
import { SettingsHero } from './SettingsHero';
import { SettingsGroup, SettingsRow, ToggleRow } from './controls';

function FlowChips({ verify }: { verify: boolean }) {
  const { t } = useTranslation();
  const arrow = <ArrowRight size={14} className="writing-flow__arrow" aria-hidden="true" />;
  return (
    <span className="writing-flow">
      <span className="writing-flow__chip">
        <Download size={14} aria-hidden="true" />
        {t('settings.writing.chipDownload')}
      </span>
      {arrow}
      <span className="writing-flow__chip">
        <HardDrive size={14} aria-hidden="true" />
        {t('settings.writing.chipWrite')}
      </span>
      <span className={`writing-flow__tail${verify ? '' : ' is-off'}`} aria-hidden={!verify}>
        {arrow}
        <span className="writing-flow__chip writing-flow__chip--ok">
          <ShieldCheck size={14} aria-hidden="true" />
          {t('settings.writing.chipVerify')}
        </span>
      </span>
    </span>
  );
}

export function WritingSection() {
  const { t } = useTranslation();
  const skipVerify = useSettingToggle(getSkipVerify, setSkipVerify, {
    fallback: SETTINGS.DEFAULTS.SKIP_VERIFY,
    toastKey: 'settings.toast.skipVerify',
  });
  const systemDrives = useSettingToggle(getAllowSystemDevices, setAllowSystemDevices, {
    fallback: SETTINGS.DEFAULTS.ALLOW_SYSTEM_DEVICES,
    toastKey: 'settings.toast.allowSystemDevices',
  });
  const verifyLoaded = skipVerify.value !== undefined;
  const verify = verifyLoaded && !skipVerify.value;

  return (
    <>
      <SettingsHero
        hero="writing"
        title={t('settings.nav.writing')}
        lead={t('settings.writing.lead')}
        heroState={{ verify: verify ? 'on' : 'off' }}
        loading={!verifyLoaded}
      />

      <SettingsGroup eyebrow={t('settings.writing.whatHappens')}>
        <div className={verifyLoaded ? undefined : 'writing-flow-row is-pending'} aria-hidden={verifyLoaded ? undefined : true}>
          <SettingsRow
            label={t(verify ? 'settings.writing.flowVerify' : 'settings.writing.flowNoVerify')}
            description={t(verify ? 'settings.writing.flowVerifyDesc' : 'settings.writing.flowNoVerifyDescNoReadback')}
            control={<FlowChips verify={verify || !verifyLoaded} />}
          />
        </div>
      </SettingsGroup>

      <SettingsGroup eyebrow={t('settings.nav.writing')}>
        <ToggleRow
          label={t('settings.writing.verifyAfter')}
          description={t('settings.writing.verifyAfterDesc')}
          checked={verify}
          loading={skipVerify.value === undefined}
          busy={skipVerify.busy}
          onChange={(on) => skipVerify.set(!on)}
        />
      </SettingsGroup>

      <SettingsGroup eyebrow={t('settings.writing.dangerZone')} danger>
        <ToggleRow
          label={t('settings.allowSystemDevices')}
          description={t('settings.allowSystemDevicesDescription')}
          checked={systemDrives.value ?? false}
          loading={systemDrives.value === undefined}
          busy={systemDrives.busy}
          onChange={systemDrives.set}
          danger
        />
      </SettingsGroup>
    </>
  );
}
