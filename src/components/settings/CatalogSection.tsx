// SPDX-License-Identifier: GPL-2.0-or-later

import { useEffect, useId, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useTranslation } from 'react-i18next';
import { FolderOpen, Check } from 'lucide-react';
import { EVENTS } from '../../config';
import { useToasts } from '../../hooks/useToasts';
import { SettingsGroup, SettingsRow } from './controls';

/** Fired on window after the catalog source changes, so the flow reloads boards and images */
export const CATALOG_CHANGED_EVENT = 'wukong-catalog-changed';

interface CatalogCheck {
  ok: boolean;
  message: string | null;
  vendors: number;
  boards: number;
  images: number;
  warnings: string[];
}

/** Optional external catalog: a URL or a folder holding catalog.json, added to the built-in one */
export function CatalogSection() {
  const { t } = useTranslation();
  const { showSuccess, showError } = useToasts();
  const inputId = useId();
  const [saved, setSaved] = useState('');
  const [value, setValue] = useState('');
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<CatalogCheck | null>(null);

  useEffect(() => {
    let alive = true;
    invoke<string>('get_catalog_source')
      .then((current) => {
        if (!alive) return;
        setSaved(current);
        setValue(current);
      })
      .catch(() => undefined);
    return () => {
      alive = false;
    };
  }, []);

  const check = async (): Promise<CatalogCheck | null> => {
    setBusy(true);
    try {
      const outcome = await invoke<CatalogCheck>('check_catalog_source', { value });
      setResult(outcome);
      return outcome;
    } catch (err) {
      setResult({ ok: false, message: String(err), vendors: 0, boards: 0, images: 0, warnings: [] });
      return null;
    } finally {
      setBusy(false);
    }
  };

  const chooseFolder = async () => {
    try {
      const folder = await invoke<string | null>('pick_catalog_folder');
      if (folder) {
        setValue(folder);
        setResult(null);
      }
    } catch (err) {
      showError(String(err));
    }
  };

  const save = async () => {
    setBusy(true);
    try {
      await invoke('set_catalog_source', { value });
      const current = await invoke<string>('get_catalog_source');
      setSaved(current);
      setValue(current);
      window.dispatchEvent(new Event(CATALOG_CHANGED_EVENT));
      window.dispatchEvent(new Event(EVENTS.CONNECTIVITY_RECHECK));
      showSuccess(t(current ? 'settings.catalog.saved' : 'settings.catalog.cleared'));
    } catch (err) {
      showError(String(err));
    } finally {
      setBusy(false);
    }
  };

  const dirty = value.trim() !== saved;

  return (
    <SettingsGroup eyebrow={t('settings.catalog.eyebrow')}>
      <SettingsRow
        label={t('settings.catalog.label')}
        description={t('settings.catalog.description')}
        controlId={inputId}
        tall
        control={
          <span style={{ display: 'flex', flexDirection: 'column', gap: 8, alignItems: 'flex-end' }}>
            <span className="pe-input is-mono" style={{ width: 360 }}>
              <input
                id={inputId}
                type="text"
                value={value}
                placeholder={t('settings.catalog.placeholder')}
                autoComplete="off"
                autoCapitalize="off"
                autoCorrect="off"
                spellCheck={false}
                onChange={(e) => {
                  setValue(e.target.value);
                  setResult(null);
                }}
              />
            </span>
            <span style={{ display: 'flex', gap: 8 }}>
              <button type="button" className="btn btn-secondary btn-pill" onClick={chooseFolder} disabled={busy}>
                <FolderOpen size={16} aria-hidden="true" />
                {t('settings.catalog.chooseFolder')}
              </button>
              <button
                type="button"
                className="btn btn-secondary btn-pill"
                onClick={check}
                disabled={busy || !value.trim()}
              >
                {t('settings.catalog.check')}
              </button>
              <button type="button" className="btn btn-primary btn-pill" onClick={save} disabled={busy || !dirty}>
                <Check size={16} aria-hidden="true" />
                {t('settings.catalog.save')}
              </button>
            </span>
          </span>
        }
      />
      {result && (
        <div className="set-row" role="status">
          <span className="set-row__text">
            <span className="set-row__label">
              {result.ok ? t('settings.catalog.checkOk') : t('settings.catalog.checkFailed')}
            </span>
            <span className="set-row__desc">
              {result.ok
                ? t('settings.catalog.counts', {
                    boards: result.boards,
                    vendors: result.vendors,
                    images: result.images,
                  })
                : result.message}
            </span>
            {result.warnings.map((warning) => (
              <span key={warning} className="set-row__desc">
                {t('settings.catalog.warning', { warning })}
              </span>
            ))}
          </span>
        </div>
      )}
    </SettingsGroup>
  );
}
