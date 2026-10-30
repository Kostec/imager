// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2026 Daniele Briguglio, superkali@armbian.com

import { useTranslation } from 'react-i18next';
import { Github, Gitlab, Link2, Power, PowerOff } from 'lucide-react';
import { AUTOCONFIG_PLACEHOLDERS, SSH_KEY_FORGES, countAuthorizedKeys, type SshKeySource } from '../../config/autoconfig';
import { accessError, type SshChoice } from '../../config/profileEditorModel';
import { wizardAccessForm } from '../../config/profileWizard';
import type { ProfileWizardState } from '../../hooks/useProfileWizard';
import { SegmentedControl } from '../settings/controls';
import { KeyImportControl, KeyImportHint } from '../settings/editor/AccessGroup';
import { FieldHint, TextArea } from '../settings/editor/EditorParts';
import { useKeyImport } from '../settings/editor/useKeyImport';
import { PasswordInput } from '../shared/PasswordInput';
import { WizardField, WizardMore, WizardToggle } from './WizardFields';

export function WizardAccessStep({ wizard, uid }: { wizard: ProfileWizardState; uid: string }) {
  const { t } = useTranslation();
  const { model, skipped, more, keyCheck, patch, toggleMore } = wizard;
  const a = model.access;
  const forge = a.source === 'link' ? null : SSH_KEY_FORGES[a.source];
  const on = a.apply && a.enabled;
  const error = accessError(wizardAccessForm(model, skipped));
  const keysError = error === 'editor.keysInvalid' || error === 'editor.keysNotImported' ? error : null;
  const importer = useKeyImport(a, (value) => patch('access', value));

  return (
    <>
      <SegmentedControl<SshChoice>
        value={a.apply ? (a.enabled ? 'on' : 'off') : null}
        ariaLabel={t('settings.autoconfig.wizard.access.title')}
        options={[
          { value: 'on', label: t('settings.autoconfig.editor.sshOn'), icon: <Power aria-hidden="true" /> },
          { value: 'off', label: t('settings.autoconfig.editor.sshOff'), icon: <PowerOff aria-hidden="true" /> },
        ]}
        onChange={(choice) => patch('access', { apply: true, enabled: choice === 'on' })}
      />
      {!a.apply && <p className="pw-line">{t('settings.autoconfig.editor.sshKeepHint')}</p>}
      {a.apply && !a.enabled && <p className="pw-line">{t('settings.autoconfig.editor.sshOffHint')}</p>}
      {on && (
        <>
          <WizardToggle
            label={t('settings.autoconfig.editor.sshPasswordAuth')}
            hint={error === 'editor.sshNoWayIn' ? <FieldHint tone="warn">{t(`settings.autoconfig.${error}`)}</FieldHint> : undefined}
            checked={a.passwordAuth}
            onChange={(passwordAuth) => patch('access', { passwordAuth })}
          />
          <WizardToggle
            label={t('settings.autoconfig.editor.sshRootLogin')}
            hint={error === 'editor.sshRootNoUser' ? <FieldHint tone="warn">{t(`settings.autoconfig.${error}`)}</FieldHint> : undefined}
            checked={a.rootLogin}
            onChange={(rootLogin) => patch('access', { rootLogin })}
          />
          <WizardField label={t('settings.autoconfig.editor.keysFrom')}>
            <SegmentedControl<SshKeySource>
              value={a.source}
              ariaLabel={t('settings.autoconfig.editor.keysFrom')}
              options={[
                { value: 'github', label: SSH_KEY_FORGES.github.label, icon: <Github aria-hidden="true" /> },
                { value: 'gitlab', label: SSH_KEY_FORGES.gitlab.label, icon: <Gitlab aria-hidden="true" /> },
                { value: 'link', label: t('settings.autoconfig.wizard.access.link'), icon: <Link2 aria-hidden="true" /> },
              ]}
              onChange={(source) => patch('access', { source })}
            />
          </WizardField>
          <WizardField
            label={forge ? t('settings.autoconfig.wizard.access.forgeUser', { forge: forge.label }) : t('settings.autoconfig.wizard.access.linkLabel')}
            htmlFor={`${uid}-keys`}
            hint={<KeyImportHint access={a} check={keyCheck} importer={importer} />}
          >
            <KeyImportControl id={`${uid}-keys`} access={a} importer={importer} onChange={(value) => patch('access', value)} />
          </WizardField>
          <WizardField
            label={t('settings.autoconfig.editor.sshKeys')}
            htmlFor={`${uid}-keytext`}
            hint={
              keysError ? (
                <FieldHint tone="warn">{t(`settings.autoconfig.${keysError}`)}</FieldHint>
              ) : (
                <FieldHint>{t('settings.autoconfig.editor.sshKeysHint', { count: countAuthorizedKeys(a.keys) })}</FieldHint>
              )
            }
          >
            <TextArea
              id={`${uid}-keytext`}
              rows={3}
              value={a.keys}
              placeholder={AUTOCONFIG_PLACEHOLDERS.SSH_KEYS}
              invalid={error === 'editor.keysInvalid'}
              onChange={(keys) => patch('access', { keys })}
            />
          </WizardField>
        </>
      )}
      <WizardMore label={t('settings.autoconfig.rootPassword')} open={!!more.access} onToggle={() => toggleMore('access')} />
      {more.access && (
        <WizardField
          label={t('settings.autoconfig.rootPassword')}
          htmlFor={`${uid}-root`}
          hint={<FieldHint>{t('settings.autoconfig.editor.rootHint')}</FieldHint>}
        >
          <PasswordInput
            id={`${uid}-root`}
            value={model.secrets.rootPassword}
            placeholder={t('settings.autoconfig.editor.rootPlaceholder')}
            onChange={(rootPassword) => patch('secrets', { rootPassword })}
          />
        </WizardField>
      )}
    </>
  );
}
