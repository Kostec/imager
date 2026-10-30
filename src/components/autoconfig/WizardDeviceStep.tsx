// SPDX-License-Identifier: GPL-2.0-or-later

import { useTranslation } from 'react-i18next';
import { Cpu } from 'lucide-react';
import { AUTOCONFIG_PLACEHOLDERS } from '../../config/autoconfig';
import { hostnameError, scriptError } from '../../config/profileEditorModel';
import type { ProfileWizardState } from '../../hooks/useProfileWizard';
import { HostnameHint, ScriptHint } from '../settings/editor/DeviceGroup';
import { FieldHint, OverlayChecks, TextArea } from '../settings/editor/EditorParts';
import { WizardField, WizardTextField } from './WizardFields';

export function WizardDeviceStep({ wizard, uid }: { wizard: ProfileWizardState; uid: string }) {
  const { t } = useTranslation();
  const { model, patch } = wizard;
  const d = model.device;

  return (
    <>
      <WizardField label={t('settings.autoconfig.editor.hostname')} htmlFor={`${uid}-host`} hint={<HostnameHint device={d} />}>
        <WizardTextField
          id={`${uid}-host`}
          mono
          value={d.hostname}
          icon={<Cpu size={15} className="pe-input__icon" aria-hidden="true" />}
          placeholder={AUTOCONFIG_PLACEHOLDERS.HOSTNAME}
          invalid={!!hostnameError(d)}
          onChange={(hostname) => patch('device', { hostname })}
        />
      </WizardField>
      <WizardField
        label={t('settings.autoconfig.editor.overlays')}
        hint={<FieldHint>{t('settings.autoconfig.editor.overlaysHint')}</FieldHint>}
      >
        <OverlayChecks selected={d.overlays} onChange={(overlays) => patch('device', { overlays })} />
      </WizardField>
      <WizardField label={t('settings.autoconfig.editor.script')} htmlFor={`${uid}-script`} hint={<ScriptHint device={d} />}>
        <TextArea
          id={`${uid}-script`}
          rows={4}
          value={d.script}
          placeholder={AUTOCONFIG_PLACEHOLDERS.USER_SCRIPT}
          invalid={!!scriptError(d)}
          onChange={(script) => patch('device', { script })}
        />
      </WizardField>
    </>
  );
}
