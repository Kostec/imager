// SPDX-License-Identifier: GPL-2.0-or-later

import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { SSH_KEY_FORGES, keyLookupSource, mergeAuthorizedKeys } from '../../../config/autoconfig';
import { keySourceFormatError, type EditorModel } from '../../../config/profileEditorModel';
import { fetchSshKeys } from '../../../hooks/useTauri';
import { getErrorMessage } from '../../../utils';
import { translateSshKeysError } from '../../../utils/errorUtils';

export interface KeyImport {
  busy: boolean;
  error: string | null;
  canImport: boolean;
  run: () => Promise<void>;
}

/** Fetches the keys on this computer and adds them to the profile: the board needs no network for them. */
export function useKeyImport(
  access: EditorModel['access'],
  onImported: (value: Pick<EditorModel['access'], 'keys' | 'keyUser' | 'keyLink'>) => void
): KeyImport {
  const { t } = useTranslation();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const forge = access.source === 'link' ? null : SSH_KEY_FORGES[access.source].label;
  const value = (access.source === 'link' ? access.keyLink : access.keyUser).trim();

  async function run() {
    setBusy(true);
    setError(null);
    try {
      const text = await fetchSshKeys(keyLookupSource(access.source), value);
      onImported({ keys: mergeAuthorizedKeys(access.keys, text), keyUser: '', keyLink: '' });
    } catch (err) {
      setError(translateSshKeysError(getErrorMessage(err), t, forge));
    } finally {
      setBusy(false);
    }
  }

  return { busy, error, canImport: !!value && !busy && keySourceFormatError(access) === null, run };
}
