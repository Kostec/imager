// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2026 Daniele Briguglio, superkali@armbian.com

import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Globe, Image } from 'lucide-react';
import { COMMON_LOCALES, KEYBOARD_LAYOUTS, getTimezones, localeLabel, timezoneOffset } from '../../../config/autoconfig';
import { localeCountry, type RegionChoice } from '../../../config/profileEditorModel';
import { SegmentedControl, SelectMenu, SettingsRow, type SelectOption } from '../controls';
import { CountryFlag, EditorSection, FieldHint } from './EditorParts';
import type { EditorGroupProps } from './types';

export function RegionGroup({ model, status, patch, keepDefaults }: EditorGroupProps) {
  const { t, i18n } = useTranslation();
  const r = model.region;
  const placeholder = t('settings.autoconfig.selectPlaceholder');

  const locales = useMemo<SelectOption<string>[]>(
    () => [
      { value: '', label: placeholder },
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
  const timezones = useMemo<SelectOption<string>[]>(() => {
    const now = new Date();
    return [
      { value: '', label: placeholder },
      ...getTimezones().map((tz) => ({ value: tz, label: tz, badge: timezoneOffset(tz, now) || undefined })),
    ];
  }, [placeholder]);

  const keyboards = useMemo<SelectOption<string>[]>(
    () => [{ value: '', label: placeholder }, ...KEYBOARD_LAYOUTS.map((k) => ({ value: k, label: k }))],
    [placeholder]
  );

  return (
    <EditorSection
      id="region"
      eyebrow={t('settings.autoconfig.wizard.steps.locale')}
      description={t('settings.autoconfig.wizard.locale.sub')}
      attention={status === 'attn'}
    >
      <SettingsRow
        label={t('settings.autoconfig.wizard.steps.locale')}
        tall={!r.apply}
        description={r.apply ? undefined : <FieldHint>{t('settings.autoconfig.armbianDefaults')}</FieldHint>}
        control={
          <SegmentedControl<RegionChoice>
            value={r.apply ? 'choose' : 'keep'}
            ariaLabel={t('settings.autoconfig.wizard.steps.locale')}
            options={[
              { value: 'keep', label: t('settings.autoconfig.wizard.locale.keep'), icon: <Image aria-hidden="true" /> },
              { value: 'choose', label: t('settings.autoconfig.wizard.locale.choose'), icon: <Globe aria-hidden="true" /> },
            ]}
            onChange={(choice) => (choice === 'keep' ? keepDefaults('region', false) : patch('region', {}))}
          />
        }
      />
      {r.apply && (
        <>
          <SettingsRow
            label={t('settings.autoconfig.wizard.locale.language')}
            control={
              <SelectMenu
                value={r.locale}
                options={locales}
                onChange={(locale) => patch('region', { locale })}
                ariaLabel={t('settings.autoconfig.wizard.locale.language')}
                searchable
                searchPlaceholder={t('settings.searchLanguage')}
              />
            }
          />
          <SettingsRow
            label={t('settings.autoconfig.timezone')}
            control={
              <SelectMenu
                value={r.timezone}
                options={timezones}
                onChange={(timezone) => patch('region', { timezone })}
                ariaLabel={t('settings.autoconfig.timezone')}
                searchable
                searchPlaceholder={t('settings.autoconfig.wizard.locale.searchTimezone')}
              />
            }
          />
          <SettingsRow
            label={t('settings.autoconfig.wizard.locale.keyboard')}
            control={
              <SelectMenu
                value={r.keyboard}
                options={keyboards}
                onChange={(keyboard) => patch('region', { keyboard })}
                ariaLabel={t('settings.autoconfig.wizard.locale.keyboard')}
              />
            }
          />
        </>
      )}
    </EditorSection>
  );
}
