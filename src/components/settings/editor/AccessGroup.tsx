// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2026 Daniele Briguglio, superkali@armbian.com

import { useTranslation } from 'react-i18next';
import { Download, Github, Gitlab, Link2, Loader2, Power, PowerOff } from 'lucide-react';
import { AUTOCONFIG_PLACEHOLDERS, SSH_KEY_FORGES, countAuthorizedKeys, type SshKeySource } from '../../../config/autoconfig';
import { accessError, keySourceFormatError, type EditorForm, type SshChoice } from '../../../config/profileEditorModel';
import type { SshKeyCheck } from '../../../hooks/useSshKeyCheck';
import { isSshKeysSourceError, translateSshKeysError } from '../../../utils/errorUtils';
import { PasswordInput } from '../../shared/PasswordInput';
import { SegmentedControl, SettingsRow, ToggleRow } from '../controls';
import { DefaultsAction, EditorSection, FieldHint, MoreRow, SavedSecretHint, TextArea, TextField } from './EditorParts';
import { fieldId, type AccessGroupProps } from './types';
import { useKeyImport, type KeyImport } from './useKeyImport';

const FORGE_ICON = { github: Github, gitlab: Gitlab } as const;

// Only a preview: the keys reach the profile with the Import button.
export function KeyCheckHint({ check, forge }: { check: SshKeyCheck; forge: string | null }) {
  const { t } = useTranslation();
  switch (check.status) {
    case 'idle':
      return null;
    case 'checking':
      return (
        <span className="pe-hint is-plain" role="status">
          <Loader2 size={12} className="spinning" aria-hidden="true" />
          <span className="pe-hint__text">{t('settings.autoconfig.keys.checking')}</span>
        </span>
      );
    case 'found':
      return <FieldHint tone="ok">{t('settings.autoconfig.keys.found', { count: check.lookup.total })}</FieldHint>;
    case 'failed':
      return (
        <FieldHint tone={isSshKeysSourceError(check.error) ? 'warn' : 'plain'}>
          {translateSshKeysError(check.error, t, forge)}
        </FieldHint>
      );
  }
}

/** The hint under the import field: its own format and failed imports first, then the preview */
export function KeyImportHint({ access, check, importer }: { access: EditorForm['access']; check: SshKeyCheck; importer: KeyImport }) {
  const { t } = useTranslation();
  const forge = access.source === 'link' ? null : SSH_KEY_FORGES[access.source].label;
  const format = keySourceFormatError(access);
  if (format) return <FieldHint tone="warn">{t(`settings.autoconfig.${format}`)}</FieldHint>;
  if (importer.error) return <FieldHint tone="warn">{importer.error}</FieldHint>;
  return <KeyCheckHint check={check} forge={forge} />;
}

export function KeyImportControl({ id, access, importer, onChange }: {
  id: string;
  access: EditorForm['access'];
  importer: KeyImport;
  onChange: (value: { keyUser: string } | { keyLink: string }) => void;
}) {
  const { t } = useTranslation();
  const forge = access.source === 'link' ? null : SSH_KEY_FORGES[access.source];
  const ForgeIcon = access.source === 'link' ? Link2 : FORGE_ICON[access.source];
  const value = forge ? access.keyUser : access.keyLink;
  return (
    <span className="pe-import">
      <TextField
        id={id}
        mono={!forge}
        value={value}
        icon={<ForgeIcon size={16} className="pe-input__icon" aria-hidden="true" />}
        placeholder={forge ? AUTOCONFIG_PLACEHOLDERS.USER_NAME : AUTOCONFIG_PLACEHOLDERS.SSH_KEYS_URL}
        invalid={!!keySourceFormatError(access)}
        onChange={(next) => onChange(forge ? { keyUser: next } : { keyLink: next })}
      />
      <button type="button" className="btn btn-secondary btn-pill" disabled={!importer.canImport} onClick={importer.run}>
        {importer.busy ? <Loader2 size={14} className="spinning" aria-hidden="true" /> : <Download size={14} aria-hidden="true" />}
        {t('settings.autoconfig.editor.importKeys')}
      </button>
    </span>
  );
}

export function AccessGroup({ model, status, uid, patch, set, keepDefaults, more, toggleMore, keyCheck }: AccessGroupProps) {
  const { t } = useTranslation();
  const a = model.access;
  const subId = fieldId(uid, 'access-more');
  const keyId = fieldId(uid, 'key');
  const keysId = fieldId(uid, 'keys');
  const forge = a.source === 'link' ? null : SSH_KEY_FORGES[a.source];
  const on = a.apply && a.enabled;
  const error = accessError(a);
  const keysError = error === 'editor.keysInvalid' || error === 'editor.keysNotImported' ? error : null;
  const importer = useKeyImport(a, (value) => patch('access', value));
  const rootSet = !!a.rootPasswordHash || !!model.secrets.rootPassword;

  return (
    <EditorSection
      id="access"
      eyebrow={t('settings.autoconfig.wizard.steps.access')}
      description={t('settings.autoconfig.wizard.access.sub')}
      attention={status === 'attn'}
      action={a.apply ? <DefaultsAction onClick={(byPointer) => keepDefaults('access', byPointer)} /> : undefined}
    >
      <SettingsRow
        label={t('settings.autoconfig.editor.howYouGetIn')}
        tall={!on}
        description={
          on ? undefined : (
            <FieldHint>{t(a.apply ? 'settings.autoconfig.editor.sshOffHint' : 'settings.autoconfig.editor.sshKeepHint')}</FieldHint>
          )
        }
        control={
          <SegmentedControl<SshChoice>
            value={a.apply ? (a.enabled ? 'on' : 'off') : null}
            ariaLabel={t('settings.autoconfig.editor.howYouGetIn')}
            options={[
              { value: 'on', label: t('settings.autoconfig.editor.sshOn'), icon: <Power aria-hidden="true" /> },
              { value: 'off', label: t('settings.autoconfig.editor.sshOff'), icon: <PowerOff aria-hidden="true" /> },
            ]}
            onChange={(choice) => patch('access', { enabled: choice === 'on' })}
          />
        }
      />
      {on && (
        <>
          <ToggleRow
            label={t('settings.autoconfig.editor.sshPasswordAuth')}
            tall={error === 'editor.sshNoWayIn'}
            description={error === 'editor.sshNoWayIn' ? <FieldHint tone="warn">{t(`settings.autoconfig.${error}`)}</FieldHint> : undefined}
            checked={a.passwordAuth}
            onChange={(passwordAuth) => patch('access', { passwordAuth })}
          />
          <ToggleRow
            label={t('settings.autoconfig.editor.sshRootLogin')}
            tall={!a.rootLogin && model.user.mode === 'later'}
            description={
              !a.rootLogin && model.user.mode === 'later' ? (
                <FieldHint tone="warn">{t('settings.autoconfig.editor.sshRootNoUser')}</FieldHint>
              ) : undefined
            }
            checked={a.rootLogin}
            onChange={(rootLogin) => patch('access', { rootLogin })}
          />
          <SettingsRow
            label={t('settings.autoconfig.editor.sshKeys')}
            controlId={keysId}
            tall
            description={
              keysError ? (
                <FieldHint tone="warn">{t(`settings.autoconfig.${keysError}`)}</FieldHint>
              ) : (
                <FieldHint>{t('settings.autoconfig.editor.sshKeysHint', { count: countAuthorizedKeys(a.keys) })}</FieldHint>
              )
            }
            control={
              <TextArea
                id={keysId}
                rows={4}
                value={a.keys}
                placeholder={AUTOCONFIG_PLACEHOLDERS.SSH_KEYS}
                invalid={error === 'editor.keysInvalid'}
                onChange={(keys) => patch('access', { keys })}
              />
            }
          />
          <SettingsRow
            label={t('settings.autoconfig.editor.keysFrom')}
            control={
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
            }
          />
          <SettingsRow
            label={forge ? t('settings.autoconfig.wizard.access.forgeUser', { forge: forge.label }) : t('settings.autoconfig.wizard.access.linkLabel')}
            controlId={keyId}
            tall
            description={<KeyImportHint access={a} check={keyCheck} importer={importer} />}
            control={<KeyImportControl id={keyId} access={a} importer={importer} onChange={(value) => patch('access', value)} />}
          />
        </>
      )}
      <MoreRow
        label={t('settings.autoconfig.editor.moreOptions')}
        value={t(rootSet ? 'settings.autoconfig.editor.rootSet' : 'settings.autoconfig.editor.rootNotSet')}
        open={more}
        controls={subId}
        onToggle={() => toggleMore('access')}
      />
      <div id={subId} className="pe-sub" hidden={!more}>
        <SettingsRow
          label={t('settings.autoconfig.rootPassword')}
          controlId={fieldId(uid, 'root')}
          tall
          description={
            a.rootPasswordHash && !model.secrets.rootPassword ? (
              <SavedSecretHint
                text={t('settings.autoconfig.editor.secretSaved')}
                onForget={() => set({ access: { ...a, rootPasswordHash: '' } })}
              />
            ) : (
              <FieldHint>{t('settings.autoconfig.editor.rootHint')}</FieldHint>
            )
          }
          control={
            <PasswordInput
              id={fieldId(uid, 'root')}
              value={model.secrets.rootPassword}
              placeholder={t('settings.autoconfig.editor.rootPlaceholder')}
              onChange={(rootPassword) => patch('secrets', { rootPassword })}
            />
          }
        />
      </div>
    </EditorSection>
  );
}
