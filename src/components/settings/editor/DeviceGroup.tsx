// SPDX-License-Identifier: GPL-2.0-or-later

import { useTranslation } from 'react-i18next';
import { Cpu } from 'lucide-react';
import { AUTOCONFIG_PLACEHOLDERS } from '../../../config/autoconfig';
import { AUTOCONFIG } from '../../../config';
import { hostnameError, scriptError, type EditorModel } from '../../../config/profileEditorModel';
import { SettingsRow } from '../controls';
import { EditorSection, FieldHint, OverlayChecks, TextArea, TextField } from './EditorParts';
import { fieldId, type EditorGroupProps } from './types';

/** Says what the name in the field means; shared with the wizard */
export function HostnameHint({ device }: { device: EditorModel['device'] }) {
  const { t } = useTranslation();
  const error = hostnameError(device);
  if (error) return <FieldHint tone="warn">{t(`settings.autoconfig.${error}`)}</FieldHint>;
  const name = device.hostname.trim();
  if (!name) return <FieldHint>{t('settings.autoconfig.editor.hostnameEmpty', { name: AUTOCONFIG.HOSTNAME_OF_IMAGE })}</FieldHint>;
  return <FieldHint tone="ok">{t('settings.autoconfig.editor.hostnameOk', { name })}</FieldHint>;
}

export function ScriptHint({ device }: { device: EditorModel['device'] }) {
  const { t } = useTranslation();
  const error = scriptError(device);
  if (error) return <FieldHint tone="warn">{t(`settings.autoconfig.${error}`)}</FieldHint>;
  return <FieldHint>{t('settings.autoconfig.editor.scriptHint')}</FieldHint>;
}

export function DeviceGroup({ model, status, uid, patch }: EditorGroupProps) {
  const { t } = useTranslation();
  const d = model.device;
  const hostId = fieldId(uid, 'hostname');
  const scriptId = fieldId(uid, 'script');

  return (
    <EditorSection
      id="device"
      eyebrow={t('settings.autoconfig.wizard.steps.device')}
      description={t('settings.autoconfig.wizard.device.sub')}
      attention={status === 'attn'}
    >
      <SettingsRow
        label={t('settings.autoconfig.editor.hostname')}
        controlId={hostId}
        tall
        description={<HostnameHint device={d} />}
        control={
          <TextField
            id={hostId}
            mono
            value={d.hostname}
            icon={<Cpu size={16} className="pe-input__icon" aria-hidden="true" />}
            placeholder={AUTOCONFIG_PLACEHOLDERS.HOSTNAME}
            invalid={!!hostnameError(d)}
            onChange={(hostname) => patch('device', { hostname })}
          />
        }
      />
      <SettingsRow
        label={t('settings.autoconfig.editor.overlays')}
        tall
        description={<FieldHint>{t('settings.autoconfig.editor.overlaysHint')}</FieldHint>}
        control={<OverlayChecks selected={d.overlays} onChange={(overlays) => patch('device', { overlays })} />}
      />
      <SettingsRow
        label={t('settings.autoconfig.editor.script')}
        controlId={scriptId}
        tall
        description={<ScriptHint device={d} />}
        control={
          <TextArea
            id={scriptId}
            rows={5}
            value={d.script}
            placeholder={AUTOCONFIG_PLACEHOLDERS.USER_SCRIPT}
            invalid={!!scriptError(d)}
            onChange={(script) => patch('device', { script })}
          />
        }
      />
    </EditorSection>
  );
}
