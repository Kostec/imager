// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2026 Daniele Briguglio, superkali@armbian.com

import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Clock, Globe, Image, Keyboard } from 'lucide-react';
import { COMMON_LOCALES, KEYBOARD_LAYOUTS, getTimezones, localeLabel, timezoneOffset } from '../../config/autoconfig';
import { localeCountry, type RegionChoice } from '../../config/profileEditorModel';
import type { ProfileWizardState } from '../../hooks/useProfileWizard';
import { SegmentedControl, SelectMenu, type SelectOption } from '../settings/controls';
import { CountryFlag } from '../settings/editor/EditorParts';
import { WizardField } from './WizardFields';

export function WizardLocaleStep({ wizard }: { wizard: ProfileWizardState }) {
  const { t, i18n } = useTranslation();
  const { model, patch } = wizard;
  const region = model.region;
  const placeholder = t('settings.autoconfig.selectPlaceholder');

  const timezones = useMemo<SelectOption<string>[]>(() => {
    const now = new Date();
    return [
      { value: '', label: placeholder, icon: <Clock aria-hidden="true" /> },
      ...getTimezones().map((tz) => ({ value: tz, label: tz, icon: <Clock aria-hidden="true" />, badge: timezoneOffset(tz, now) || undefined })),
    ];
  }, [placeholder]);
  const locales = useMemo<SelectOption<string>[]>(
    () => [
      { value: '', label: placeholder, icon: <Globe aria-hidden="true" /> },
      ...COMMON_LOCALES.map((l) => {
        const country = localeCountry(l);
        return {
          value: l,
          label: localeLabel(l, i18n.language),
          icon: country ? <CountryFlag code={country} /> : <Globe aria-hidden="true" />,
          badge: l.split('.')[0],
        };
      }),
    ],
    [placeholder, i18n.language]
  );

  const keyboards = useMemo<SelectOption<string>[]>(
    () => [
      { value: '', label: placeholder, icon: <Keyboard aria-hidden="true" /> },
      ...KEYBOARD_LAYOUTS.map((k) => ({ value: k, label: k, icon: <Keyboard aria-hidden="true" /> })),
    ],
    [placeholder]
  );

  return (
    <>
      <SegmentedControl<RegionChoice>
        value={region.apply ? 'choose' : 'keep'}
        ariaLabel={t('settings.autoconfig.wizard.locale.title')}
        options={[
          { value: 'keep', label: t('settings.autoconfig.wizard.locale.keep'), icon: <Image aria-hidden="true" /> },
          { value: 'choose', label: t('settings.autoconfig.wizard.locale.choose'), icon: <Globe aria-hidden="true" /> },
        ]}
        onChange={(choice) => patch('region', { apply: choice === 'choose' })}
      />
      {!region.apply ? (
        <p className="pw-line">{t('settings.autoconfig.wizard.pending.locale')}</p>
      ) : (
        <div className="pw-two">
          <WizardField label={t('settings.autoconfig.wizard.locale.language')}>
            <SelectMenu
              value={region.locale}
              options={locales}
              onChange={(locale) => patch('region', { locale })}
              ariaLabel={t('settings.autoconfig.wizard.locale.language')}
              searchable
              searchPlaceholder={t('settings.searchLanguage')}
            />
          </WizardField>
          <WizardField label={t('settings.autoconfig.timezone')}>
            <SelectMenu
              value={region.timezone}
              options={timezones}
              onChange={(timezone) => patch('region', { timezone })}
              ariaLabel={t('settings.autoconfig.timezone')}
              searchable
              searchPlaceholder={t('settings.autoconfig.wizard.locale.searchTimezone')}
            />
          </WizardField>
          <WizardField label={t('settings.autoconfig.wizard.locale.keyboard')}>
            <SelectMenu
              value={region.keyboard}
              options={keyboards}
              onChange={(keyboard) => patch('region', { keyboard })}
              ariaLabel={t('settings.autoconfig.wizard.locale.keyboard')}
            />
          </WizardField>
        </div>
      )}
    </>
  );
}
