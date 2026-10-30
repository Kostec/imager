// SPDX-License-Identifier: GPL-2.0-or-later

import { Archive, Disc3, FolderOpen } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import { getImagesForBoard } from '../../hooks/useTauri';
import { useAsyncData } from '../../hooks/useAsyncData';
import type { BoardInfo, ImageInfo } from '../../types';
import { OsPanel } from './OsPanel';

interface ImagePanelProps {
  board: BoardInfo;
  onChooseFile: () => void;
  onOpenCacheManager: () => void;
  onSelect: (image: ImageInfo) => void;
}

/** Image step: a file from disk is always on offer; catalog images follow when there are any. */
export function ImagePanel({ board, onChooseFile, onOpenCacheManager, onSelect }: ImagePanelProps) {
  const { t } = useTranslation();
  const { data: images } = useAsyncData<ImageInfo[]>(() => getImagesForBoard(board.slug), [board.slug]);
  // An external catalog that failed to load leaves the built-in one; say why.
  const { data: catalogError } = useAsyncData<string | null>(() => invoke<string | null>('get_catalog_error'), [images]);

  return (
    <div className="mfr-panel image-panel">
      <div className="image-panel__choice">
        <Disc3 className="image-panel__icon" size={44} strokeWidth={1.5} aria-hidden="true" />
        <h2 className="image-panel__title">{t('home.chooseImageTitle')}</h2>
        <p className="image-panel__hint">{t('home.chooseImageHint')}</p>
        {catalogError && (
          <p className="image-panel__hint" role="alert">
            {t('home.catalogError', { reason: catalogError })}
          </p>
        )}
        <div className="image-panel__actions">
          <button type="button" className="btn btn-primary btn-pill" onClick={onChooseFile}>
            <FolderOpen size={16} aria-hidden="true" />
            {t('home.useCustomImage')}
          </button>
          <button type="button" className="btn btn-secondary btn-pill" onClick={onOpenCacheManager}>
            <Archive size={16} aria-hidden="true" />
            {t('home.cachedImages')}
          </button>
        </div>
      </div>
      {!!images?.length && <OsPanel board={board} onSelect={onSelect} />}
    </div>
  );
}
