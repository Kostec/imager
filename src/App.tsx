// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2025-2026 Daniele Briguglio, superkali@armbian.com

import { useState, useCallback, useEffect, useRef, lazy, Suspense } from 'react';
import { useTranslation } from 'react-i18next';
import { Header, HomePage } from './components/layout';
import { CATALOG_CHANGED_EVENT } from './components/settings/CatalogSection';
import { FlashProgress } from './components/flash';
import { CacheManagerModal, SettingsPage } from './components/settings';
import { getBoards, selectCustomImage, classifyCustomImage, logInfo, logWarn, checkNeedsDecompression, decompressCustomImage } from './hooks/useTauri';
import { useDeviceMonitor } from './hooks/useDeviceMonitor';
import { useConnectivity } from './hooks/useConnectivity';
import { ToastProvider, useToasts } from './hooks/useToasts';
import { useModalExitAnimation } from './hooks/useModalExitAnimation';
import { useMotion } from './contexts/MotionContext';
import { getAutoconfigProfile } from './hooks/useSettings';
import { EVENTS, IMAGE_VARIANT, UI, SETTINGS_VIEW, WUKONG_BOARD } from './config';
import { IMAGE_FORMAT, isEdlImage } from './types';
import { presetIsEmpty } from './config/autoconfig';
import { buildLocalImage, focusAfterInput, DEFAULT_COLOR } from './utils';
import { imageErrorKey } from './utils/errorUtils';
import type { ImageInfo, BlockDevice, BoardInfo, Manufacturer, SelectionStep, AutoconfigConfig, CustomImageInfo, FlashExit, LeaveGuard, SettingsView } from './types';
import './styles/index.css';

// Debug-only panel; the define folds to false in release builds, which drops the chunk.
const DevScenarios = __DEV_SCENARIOS__ ? lazy(() => import('./components/dev/DevScenarios')) : null;

function App() {
  return (
    <ToastProvider>
      <AppContent />
    </ToastProvider>
  );
}

/** Main application content, must be inside ToastProvider to use useToasts() */
function AppContent() {
  const { t } = useTranslation();
  const [isFlashing, setIsFlashing] = useState(false);
  const [settledFlashExit, setSettledFlashExit] = useState<FlashExit | null>(null);
  const [selectionEpoch, setSelectionEpoch] = useState(0);
  // Boards of the active catalog; the built-in catalog is the one Wukong board.
  const [catalogBoards, setCatalogBoards] = useState<BoardInfo[] | null>(null);
  const [pickedManufacturer, setPickedManufacturer] = useState<Manufacturer | null>(null);
  const [pickedBoard, setPickedBoard] = useState<BoardInfo | null>(null);
  const boards = catalogBoards?.length ? catalogBoards : [WUKONG_BOARD];
  // The pickers only appear when there is something to choose.
  const showManufacturerStep = new Set(boards.map((b) => b.vendor)).size > 1;
  const showBoardStep = boards.length > 1;
  const selectedManufacturer: Manufacturer | null =
    pickedManufacturer ??
    (showManufacturerStep
      ? null
      : { id: boards[0].vendor, name: boards[0].vendor_name, color: DEFAULT_COLOR, boardCount: boards.length });
  const selectedBoard: BoardInfo | null = pickedBoard ?? (showBoardStep ? null : boards[0]);
  const [selectedImage, setSelectedImage] = useState<ImageInfo | null>(null);
  const [selectedDevice, setSelectedDevice] = useState<BlockDevice | null>(null);
  // Opt-in autoconfig profile id picked at flash time; null means unchanged behaviour.
  const [selectedProfileId, setSelectedProfileId] = useState<string | null>(null);
  const [autoconfig, setAutoconfig] = useState<AutoconfigConfig | null>(null);

  const { showSuccess, showError } = useToasts();

  const { isOnline } = useConnectivity();
  const prevOnlineRef = useRef<boolean | null>(null);

  const [showCacheManager, setShowCacheManager] = useState(false);

  const { reduced } = useMotion();
  const [settings, setSettings] = useState<SettingsView | null>(null);
  const leaveGuardRef = useRef<LeaveGuard | null>(null);
  const gearRef = useRef<HTMLButtonElement>(null);
  const focusReturnRef = useRef<HTMLElement | null>(null);
  const restoreFocusRef = useRef(false);
  const pointerInputRef = useRef(false);

  const [homeReturning, setHomeReturning] = useState(false);

  const dropSettings = useCallback(() => {
    leaveGuardRef.current = null;
    setSettings(null);
  }, []);
  const startHomeReturn = useCallback(() => setHomeReturning(!reduced), [reduced]);
  const { isExiting: settingsExiting, handleClose: closeSettingsNow } = useModalExitAnimation({
    onClose: dropSettings,
    onExiting: startHomeReturn,
    duration: reduced ? 0 : UI.SETTINGS_PAGE.EXIT_MS,
  });

  // The guard shows the editor's own "Discard changes?" dialog and resolves false on Cancel.
  const guardedLeave = useCallback(async (leave: () => void) => {
    const guard = leaveGuardRef.current;
    if (guard && !(await guard())) return;
    leave();
  }, []);

  const registerLeaveGuard = useCallback((guard: LeaveGuard | null) => {
    leaveGuardRef.current = guard;
  }, []);

  const openSettings = useCallback(
    (view: SettingsView) => {
      if (!settings) {
        focusReturnRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
        restoreFocusRef.current = true;
        setHomeReturning(false);
        setSettings(view);
        return;
      }
      guardedLeave(() => {
        leaveGuardRef.current = null;
        setSettings(view);
      });
    },
    [settings, guardedLeave]
  );

  const requestCloseSettings = useCallback(() => {
    guardedLeave(closeSettingsNow);
  }, [guardedLeave, closeSettingsNow]);

  const requestSettingsView = useCallback(
    (view: SettingsView) => {
      guardedLeave(() => {
        leaveGuardRef.current = null;
        setSettings((current) => (current ? view : current));
      });
    },
    [guardedLeave]
  );

  // Picking a cached image to reuse sends the user straight back to the flow.
  useEffect(() => {
    if (!settings) return;
    window.addEventListener(EVENTS.CACHE_IMAGE_REUSE, closeSettingsNow);
    return () => window.removeEventListener(EVENTS.CACHE_IMAGE_REUSE, closeSettingsNow);
  }, [settings, closeSettingsNow]);

  const settingsCovering = !!settings && !settingsExiting;

  // Last input modality, so focus returned after a mouse close does not draw a keyboard ring.
  useEffect(() => {
    const onPointer = () => {
      pointerInputRef.current = true;
    };
    const onKey = () => {
      pointerInputRef.current = false;
    };
    window.addEventListener('pointerdown', onPointer, true);
    window.addEventListener('keydown', onKey, true);
    return () => {
      window.removeEventListener('pointerdown', onPointer, true);
      window.removeEventListener('keydown', onKey, true);
    };
  }, []);

  // Runs after the commit that lifts inert from the home layer, so its controls can take focus again.
  useEffect(() => {
    if (settings || !restoreFocusRef.current) return;
    restoreFocusRef.current = false;
    const target = focusReturnRef.current;
    focusReturnRef.current = null;
    const next = target?.isConnected && target !== document.body ? target : gearRef.current;
    if (!next) return;
    focusAfterInput(next, pointerInputRef.current);
  }, [settings]);

  // Clear selected device if disconnected, only when not flashing
  useDeviceMonitor(
    selectedDevice,
    !!selectedImage && isEdlImage(selectedImage),
    useCallback(() => setSelectedDevice(null), []),
    !isFlashing
  );

  // Receive the opt-in autoconfig profile id chosen in the DevicePanel confirm view
  useEffect(() => {
    const handler = (e: Event) => {
      const id = (e as CustomEvent<{ id: string | null }>).detail?.id ?? null;
      setSelectedProfileId(id);
    };
    window.addEventListener(EVENTS.AUTOCONFIG_PROFILE_SELECTED, handler);
    return () => window.removeEventListener(EVENTS.AUTOCONFIG_PROFILE_SELECTED, handler);
  }, []);

  // Resolve the picked profile id to its config; null when no profile is selected
  useEffect(() => {
    if (!selectedProfileId) {
      // eslint-disable-next-line react-hooks/set-state-in-effect -- drop the config along with the pick
      setAutoconfig(null);
      return;
    }
    let cancelled = false;
    getAutoconfigProfile(selectedProfileId)
      .then((profile) => {
        // An all-defaults profile renders an empty preset: nothing to prepare or inject
        const config = profile?.config;
        if (!cancelled) setAutoconfig(config && !presetIsEmpty(config) ? config : null);
      })
      .catch(() => {
        if (!cancelled) setAutoconfig(null);
      });
    return () => {
      cancelled = true;
    };
  }, [selectedProfileId]);

  // Boards come from the catalog source; a change of source starts the flow over.
  const [catalogEpoch, setCatalogEpoch] = useState(0);
  useEffect(() => {
    let cancelled = false;
    getBoards()
      .then((list) => !cancelled && setCatalogBoards(list))
      .catch(() => !cancelled && setCatalogBoards(null));
    return () => {
      cancelled = true;
    };
  }, [catalogEpoch]);

  useEffect(() => {
    const handler = () => {
      setPickedManufacturer(null);
      setPickedBoard(null);
      setSelectedImage(null);
      setSelectedDevice(null);
      setSelectedProfileId(null);
      setCatalogEpoch((epoch) => epoch + 1);
      setSelectionEpoch((epoch) => epoch + 1);
    };
    window.addEventListener(CATALOG_CHANGED_EVENT, handler);
    return () => window.removeEventListener(CATALOG_CHANGED_EVENT, handler);
  }, []);

  // Toast on reconnect, but not on initial mount
  useEffect(() => {
    if (prevOnlineRef.current === false && isOnline) {
      showSuccess(t('home.connectionRestored'));
    }
    prevOnlineRef.current = isOnline;
  }, [isOnline, showSuccess, t]);

  // Clear a step and all downstream selections, which become invalid when it changes
  const resetSelectionsFrom = useCallback((step: SelectionStep) => {
    if (step === 'manufacturer') setPickedManufacturer(null);
    if (step === 'manufacturer' || step === 'board') setPickedBoard(null);
    if (step !== 'device') setSelectedImage(null);
    setSelectedDevice(null);
    // The profile picker belongs to the device step; drop the opt-in selection.
    setSelectedProfileId(null);
  }, []);

  // Reuse a cached image from the Cache Manager
  useEffect(() => {
    const handler = async (e: Event) => {
      const { path, filename, size } = (e as CustomEvent).detail;

      logInfo('app', `Reusing cached image: ${filename}`);

      let imagePath = path;
      try {
        const needsDecompress = await checkNeedsDecompression(path);
        if (needsDecompress) {
          logInfo('app', `Cached image needs decompression: ${filename}`);
          imagePath = await decompressCustomImage(path);
        }
      } catch (err) {
        logWarn('app', `Failed to check/decompress cached image: ${err}`);
        showError(t(imageErrorKey(err) ?? 'error.imageDecompressFailed'));
        return;
      }

      const cachedImage = buildLocalImage({
        variant: IMAGE_VARIANT.CACHED,
        name: filename,
        size,
        path: imagePath,
        format: IMAGE_FORMAT.SD,
      });

      resetSelectionsFrom('image');
      setSelectedImage(cachedImage);
    };

    window.addEventListener(EVENTS.CACHE_IMAGE_REUSE, handler);
    return () => window.removeEventListener(EVENTS.CACHE_IMAGE_REUSE, handler);
  }, [t, showError, resetSelectionsFrom]);

  function handleManufacturerSelect(manufacturer: Manufacturer) {
    setPickedManufacturer(manufacturer);
    resetSelectionsFrom('board');
  }

  function handleBoardSelect(board: BoardInfo) {
    setPickedBoard(board);
    resetSelectionsFrom('image');
  }

  function handleImageSelect(image: ImageInfo) {
    setSelectedImage(image);
    resetSelectionsFrom('device');
  }

  // Reveals the inline confirm summary; does not start flashing yet
  function handleDeviceSelect(device: BlockDevice) {
    setSelectedDevice(device);
  }

  // Confirm the inline summary: begin flashing the picked device
  function handleConfirmFlash() {
    setIsFlashing(true);
  }

  // Cancel the inline confirm: drop back to the device list
  function handleClearDevice() {
    setSelectedDevice(null);
    setSelectedProfileId(null);
  }

  async function handleCustomImage() {
    try {
      const result = await selectCustomImage();
      if (result) {
        await applyCustomImage(result);
      }
    } catch (err) {
      logWarn('app', `Failed to select custom image: ${err}`);
      showError(t(imageErrorKey(err) ?? 'custom.selectError'));
    }
  }

  async function applyCustomImage(result: CustomImageInfo) {
    // One backend call classifies the picked file; only QDL archives are refused here.
    const { is_qdl: isQdl, supports_autoconfig: supportsAutoconfig } = await classifyCustomImage(result.path).catch(() => ({
      is_qdl: false,
      supports_autoconfig: false,
    }));
    if (isQdl) {
      logInfo('app', `Refused QDL archive: ${result.name}`);
      showError(t('custom.qdlUnsupported'));
      return;
    }

    const customImage = buildLocalImage({
      variant: IMAGE_VARIANT.CUSTOM,
      name: result.name,
      size: result.size,
      path: result.path,
      format: IMAGE_FORMAT.BLOCK,
      storage: null,
      supportsAutoconfig,
    });

    resetSelectionsFrom('image');
    setSelectedImage(customImage);
  }

  function handleComplete() {
    setIsFlashing(false);
    resetSelectionsFrom('image');
  }

  function handleBackFromFlash() {
    setIsFlashing(false);
    setSelectedDevice(null); // Allow re-selection
    setSelectedProfileId(null);
  }

  function handleReset() {
    resetSelectionsFrom('image');
  }

  const handleFlashSettled = useCallback((exit: FlashExit | null) => {
    setSettledFlashExit(() => exit);
  }, []);

  async function handleRestartSelection() {
    if (isFlashing) {
      if (!settledFlashExit) return;
      await settledFlashExit();
    }
    resetSelectionsFrom('image');
    setSelectionEpoch((epoch) => epoch + 1);
  }

  function handleNavigateToStep(step: SelectionStep) {
    // Reset from this step onward so it becomes the active inline panel
    resetSelectionsFrom(step);
  }

  return (
    <div className="app">
      {/* macOS overlay titlebar drag strip; reserved --macos-titlebar-h stays clear of the traffic-light controls */}
      <div className="titlebar-drag" data-tauri-drag-region />
      <Header
        selectedImage={selectedImage}
        selectedDevice={selectedDevice}
        selectedManufacturer={selectedManufacturer}
        selectedBoard={selectedBoard}
        showManufacturerStep={showManufacturerStep}
        showBoardStep={showBoardStep}
        onReset={handleReset}
        onNavigateToStep={handleNavigateToStep}
        isFlashing={isFlashing}
        hideSettings={isFlashing}
        settingsOpen={!!settings && !settingsExiting}
        onOpenSettings={() => openSettings(SETTINGS_VIEW.GENERAL)}
        onCloseSettings={requestCloseSettings}
        settingsButtonRef={gearRef}
      />

      <main className={`main-content${isFlashing ? '' : ' main-content--home'}`}>
        {isFlashing ? (
          selectedImage && selectedDevice && (
            <FlashProgress
              board={selectedBoard ?? WUKONG_BOARD}
              image={selectedImage}
              device={selectedDevice}
              autoconfig={autoconfig}
              onComplete={handleComplete}
              onBack={handleBackFromFlash}
              onSettledChange={DevScenarios ? handleFlashSettled : undefined}
            />
          )
        ) : (
          <>
          <div
            className={`home-layer${settingsCovering ? ' is-covered' : homeReturning ? ' is-returning' : ''}`}
            onTransitionEnd={(e) => {
              if (e.target === e.currentTarget && e.propertyName === 'transform') setHomeReturning(false);
            }}
            onTransitionCancel={(e) => {
              if (e.target === e.currentTarget && e.propertyName === 'transform') setHomeReturning(false);
            }}
            inert={settingsCovering || undefined}
            aria-hidden={settingsCovering || undefined}
          >
          <HomePage
            key={selectionEpoch}
            selectedManufacturer={selectedManufacturer}
            selectedBoard={selectedBoard}
            showManufacturerStep={showManufacturerStep}
            showBoardStep={showBoardStep}
            onChooseManufacturer={() => resetSelectionsFrom('manufacturer')}
            onChooseBoard={() => resetSelectionsFrom('board')}
            onSelectManufacturer={handleManufacturerSelect}
            onSelectBoard={handleBoardSelect}
            selectedImage={selectedImage}
            selectedDevice={selectedDevice}
            onChooseImage={() => resetSelectionsFrom('image')}
            onChooseDevice={() => resetSelectionsFrom('device')}
            onChooseCustomImage={handleCustomImage}
            onOpenCacheManager={() => setShowCacheManager(true)}
            onSelectImage={handleImageSelect}
            onSelectDevice={handleDeviceSelect}
            onConfirmDevice={handleConfirmFlash}
            onClearDevice={handleClearDevice}
          />
          </div>
          {settings && (
            <SettingsPage
              view={settings}
              exiting={settingsExiting}
              onViewChange={requestSettingsView}
              onClose={requestCloseSettings}
              registerLeaveGuard={registerLeaveGuard}
            />
          )}
          </>
        )}
      </main>

      {/* Standalone cache manager, reached from the image step */}
      <CacheManagerModal
        isOpen={showCacheManager}
        onClose={() => setShowCacheManager(false)}
      />

      {DevScenarios && (
        <Suspense fallback={null}>
          <DevScenarios
            hidden={false}
            isFlashing={isFlashing}
            isConfirming={!isFlashing && selectedDevice !== null}
            onUseCustomImage={applyCustomImage}
            onResetFlow={isFlashing && !settledFlashExit ? null : handleRestartSelection}
            onRunArmbianDetection={null}
          />
        </Suspense>
      )}
    </div>
  );
}

export default App;
