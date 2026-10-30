// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2025-2026 Daniele Briguglio, superkali@armbian.com

import { useState, useEffect, type ReactNode, type ComponentType } from 'react';
import { Cpu, CircuitBoard, Database, Factory, HardDrive, Usb, FolderOpen, Check, ArrowRight, Lock } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { getCachedBoardImage } from '../../hooks/useTauri';
import { FEATURES, IMAGE_VARIANT } from '../../config';
import { formatImageIdentity, formatKernelLabel } from '../../utils';
import type { BoardInfo, ImageInfo, BlockDevice, Manufacturer } from '../../types';
import { deriveFlashMethod, isEdlImage } from '../../types';
import { MarqueeText, BoardImage } from '../shared';
import { ManufacturerPanel } from './ManufacturerPanel';
import { BoardPanel } from './BoardPanel';
import { ImagePanel } from './ImagePanel';
import { DevicePanel } from './DevicePanel';

interface HomePageProps {
  selectedManufacturer: Manufacturer | null;
  /** Null until the user picks one, when the catalog offers several */
  selectedBoard: BoardInfo | null;
  /** The catalog offers several manufacturers */
  showManufacturerStep: boolean;
  /** The catalog offers several boards */
  showBoardStep: boolean;
  onChooseManufacturer: () => void;
  onChooseBoard: () => void;
  onSelectManufacturer: (manufacturer: Manufacturer) => void;
  onSelectBoard: (board: BoardInfo) => void;
  selectedImage: ImageInfo | null;
  selectedDevice: BlockDevice | null;
  onChooseImage: () => void;
  onChooseDevice: () => void;
  onChooseCustomImage: () => void;
  onOpenCacheManager: () => void;
  /** Select a catalog image directly from the inline browser. */
  onSelectImage: (image: ImageInfo) => void;
  /** Pick a storage device (populates the steps, shows the confirm summary). */
  onSelectDevice: (device: BlockDevice) => void;
  /** Confirm the inline storage summary and start flashing. */
  onConfirmDevice: () => void;
  /** Cancel the inline storage summary (back to the device list). */
  onClearDevice: () => void;
}

/** Visual/interaction state of a step. */
type StepState = 'done' | 'active' | 'locked' | 'readonly';

/** Descriptor for a single step, shared by the sidebar nav and the focus panel. */
interface Step {
  key: string;
  index: number;
  icon: ComponentType<{ size?: number }>;
  label: string;
  cta: string;
  /** Selected value (already-localized node), absent when nothing chosen yet. */
  value?: ReactNode;
  /** Secondary muted line (size, image count, kernel branch). */
  meta?: ReactNode;
  state: StepState;
  /** The final write action, rendered with accent styling. */
  primary?: boolean;
  onClick?: () => void;
  /** Inline selector shown full-panel when this step is active (e.g. manufacturer grid). */
  panel?: ReactNode;
}

/** Sidebar navigation item. */
function SideStep({ step }: { step: Step }) {
  const { index, icon: Icon, label, value, state, onClick } = step;
  const isDone = state === 'done' || state === 'readonly';
  const clickable = (state === 'active' || state === 'done') && !!onClick;

  return (
    <button
      type="button"
      className={`side-step is-${state}`}
      onClick={clickable ? onClick : undefined}
      disabled={!clickable}
      aria-current={state === 'active'}
    >
      <span className="side-step__badge">{isDone ? <Check size={13} strokeWidth={3} /> : index}</span>
      <span className="side-step__icon"><Icon size={18} /></span>
      <span className="side-step__text">
        <span className="side-step__label">{label}</span>
        {value ? (
          <span className="side-step__value">{value}</span>
        ) : (
          <span className="side-step__value is-empty">{step.cta}</span>
        )}
      </span>
      {state === 'locked' && <Lock size={14} className="side-step__lock" />}
    </button>
  );
}

/** OS identity string (e.g. the image file name); shared with the flash header. */
function osLabelText(image: ImageInfo, t: (key: string) => string): string {
  return formatImageIdentity(image, t).title;
}

/** OS title/meta builders, reused across branches. */
function osTitle(image: ImageInfo, t: (key: string) => string): ReactNode {
  return <MarqueeText text={osLabelText(image, t)} className="val-text" />;
}
function osMeta(image: ImageInfo, t: (key: string) => string): ReactNode {
  return formatImageIdentity(image, t).meta;
}

/** Last step is a USB "device" (icon + wording) for EDL/QDL images, a "storage" drive otherwise. */
function deviceStepMeta(
  image: ImageInfo | null,
  t: (key: string) => string
): { icon: ComponentType<{ size?: number }>; label: string; cta: string } {
  return image && isEdlImage(image)
    ? { icon: Usb, label: t('home.device'), cta: t('home.chooseDevice') }
    : { icon: HardDrive, label: t('home.storage'), cta: t('home.chooseStorage') };
}

/** Step sidebar + focus panel that highlights the active step and previews the board. */
function SplitLayout({
  boardName,
  steps,
  boardImage,
  footer,
  fullBleed = false,
}: {
  boardName: string;
  steps: Step[];
  boardImage: string | null;
  footer?: ReactNode;
  /** Collapse the sidebar and expand the panel full-width (e.g. confirm view). */
  fullBleed?: boolean;
}) {
  // The active step drives the focus panel; fall back to the last step when done.
  const active = steps.find((s) => s.state === 'active') ?? steps[steps.length - 1];
  const ActiveIcon = active.icon;
  const completed = steps.filter((s) => s.state === 'done' || s.state === 'readonly');

  return (
    <div className="home-page home-split">
      <div className={`split${fullBleed ? ' is-full' : ''}`}>
        <aside className="split-nav">
          <div className="split-nav__board">
            <Cpu size={18} aria-hidden="true" />
            <span>{boardName}</span>
          </div>
          <nav className="split-nav__list">
            {steps.map((step) => (
              <SideStep key={step.key} step={step} />
            ))}
          </nav>
          <div className="split-nav__bottom">
            {footer && <div className="split-nav__foot">{footer}</div>}
          </div>
        </aside>

        <section className={`split-main${active.panel ? ' is-panel' : ''}`}>
          {active.panel ? (
            active.panel
          ) : (
          <div className="focus">
            <div className="focus-visual">
              {boardImage ? (
                // BoardImage owns the logo watermark fallback when a photo fails.
                <BoardImage src={boardImage} alt="" />
              ) : (
                <span className="focus-visual__icon"><ActiveIcon size={44} /></span>
              )}
            </div>

            <span className="focus-kicker">{active.label}</span>
            <h2 className="focus-title">{active.value ?? active.cta}</h2>

            <button
              type="button"
              className={`focus-cta${active.primary ? ' is-primary' : ''}`}
              onClick={active.onClick}
              disabled={active.state === 'locked' || active.state === 'readonly'}
            >
              {active.cta}
              <ArrowRight size={17} />
            </button>

            {completed.length > 0 && (
              <ul className="focus-summary">
                {completed.map((s) => (
                  <li key={s.key}>
                    <Check size={13} strokeWidth={3} />
                    <span className="focus-summary__label">{s.label}</span>
                    <span className="focus-summary__value">{s.value}</span>
                  </li>
                ))}
              </ul>
            )}
          </div>
          )}
        </section>
      </div>
    </div>
  );
}

/** Main selection page: manufacturer and board when the catalog offers a choice, then image and storage. */
export function HomePage({
  selectedManufacturer,
  selectedBoard,
  showManufacturerStep,
  showBoardStep,
  onChooseManufacturer,
  onChooseBoard,
  onSelectManufacturer,
  onSelectBoard,
  selectedImage,
  selectedDevice,
  onChooseImage,
  onChooseDevice,
  onChooseCustomImage,
  onOpenCacheManager,
  onSelectImage,
  onSelectDevice,
  onConfirmDevice,
  onClearDevice,
}: HomePageProps) {
  const { t } = useTranslation();
  const isCached = selectedImage?.image_variant === IMAGE_VARIANT.CACHED;

  // A catalog may carry a board photo; without one the panels show the neutral icon.
  const previewSlug = selectedBoard?.slug ?? null;
  const [boardImage, setBoardImage] = useState<string | null>(null);
  useEffect(() => {
    if (!previewSlug) return;
    let alive = true;
    getCachedBoardImage(previewSlug)
      .then((uri) => alive && setBoardImage(uri))
      .catch(() => alive && setBoardImage(null));
    return () => {
      alive = false;
    };
  }, [previewSlug]);

  const deviceValue = selectedDevice ? (
    <MarqueeText text={selectedDevice.model || selectedDevice.name} className="val-text" />
  ) : undefined;
  const deviceMeta = selectedDevice ? selectedDevice.size_formatted : null;
  const dStep = deviceStepMeta(selectedImage, t);

  // Selections shown in the inline storage confirm summary.
  const deviceSummary = [
    selectedBoard && { label: t('home.board'), value: selectedBoard.name },
    selectedImage && {
      label: t('home.operatingSystem'),
      value: osLabelText(selectedImage, t),
      sub: formatKernelLabel(selectedImage),
    },
  ].filter(Boolean) as { label: string; value: string; sub?: string | null }[];

  // Inline storage panel; it picks list vs. confirm view from `selectedDevice`.
  const renderDevicePanel = () => (
    <DevicePanel
      flashMethod={selectedImage ? deriveFlashMethod(selectedImage) : undefined}
      summary={deviceSummary}
      boardImage={boardImage}
      boardSlug={selectedBoard?.slug ?? ''}
      boardName={selectedBoard?.name ?? ''}
      selectedDevice={selectedDevice}
      onSelect={onSelectDevice}
      onConfirm={onConfirmDevice}
      onCancel={onClearDevice}
      supportsAutoconfig={FEATURES.AUTOCONFIG && (selectedImage?.supports_autoconfig ?? false)}
    />
  );

  // A picked image stands in for the choices before it (a cached image needs no board).
  const manufacturerDone = !!selectedManufacturer || !!selectedImage;
  const boardDone = !!selectedBoard || !!selectedImage;

  const steps: Step[] = [];
  if (showManufacturerStep) {
    steps.push({
      key: 'manufacturer',
      index: steps.length + 1,
      icon: Factory,
      label: t('home.manufacturer'),
      cta: t('home.chooseBrand'),
      value: selectedManufacturer?.name,
      state: manufacturerDone ? 'done' : 'active',
      onClick: onChooseManufacturer,
      panel: manufacturerDone ? undefined : <ManufacturerPanel onSelect={onSelectManufacturer} />,
    });
  }
  if (showBoardStep) {
    steps.push({
      key: 'board',
      index: steps.length + 1,
      icon: CircuitBoard,
      label: t('home.board'),
      cta: t('home.chooseBoard'),
      value: selectedBoard?.name,
      state: !manufacturerDone ? 'locked' : boardDone ? 'done' : 'active',
      onClick: onChooseBoard,
      panel:
        selectedManufacturer && !boardDone ? (
          <BoardPanel manufacturer={selectedManufacturer} onSelect={onSelectBoard} />
        ) : undefined,
    });
  }
  steps.push(
    {
      key: 'image',
      index: steps.length + 1,
      icon: Database,
      label: t('home.operatingSystem'),
      cta: t('home.chooseOs'),
      value: selectedImage ? osTitle(selectedImage, t) : undefined,
      meta: selectedImage ? osMeta(selectedImage, t) : null,
      state: selectedImage ? 'done' : boardDone ? 'active' : 'locked',
      onClick: onChooseImage,
      panel: selectedImage || !selectedBoard ? undefined : (
        <ImagePanel
          board={selectedBoard}
          onChooseFile={onChooseCustomImage}
          onOpenCacheManager={onOpenCacheManager}
          onSelect={onSelectImage}
        />
      ),
    },
    {
      key: 'device',
      index: steps.length + 2,
      icon: dStep.icon,
      label: dStep.label,
      cta: dStep.cta,
      value: deviceValue,
      meta: deviceMeta,
      state: !selectedImage ? 'locked' : selectedDevice ? 'done' : 'active',
      primary: !!selectedImage && !selectedDevice,
      onClick: onChooseDevice,
      panel: selectedImage ? renderDevicePanel() : undefined,
    }
  );

  return (
    <SplitLayout
      boardName={selectedBoard?.name ?? selectedManufacturer?.name ?? 'Wukong Imager'}
      steps={steps}
      boardImage={boardImage}
      fullBleed={!!selectedDevice}
      footer={
        selectedImage?.is_custom ? (
          <div className="split-nav__actions">
            <button type="button" className="split-link" onClick={isCached ? onOpenCacheManager : onChooseCustomImage}>
              <FolderOpen size={15} />
              {isCached ? t('home.changeCachedImage') : t('home.changeCustomImage')}
            </button>
          </div>
        ) : undefined
      }
    />
  );
}
