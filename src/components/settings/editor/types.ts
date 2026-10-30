// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2026 Daniele Briguglio, superkali@armbian.com

import type { EditorForm, GroupStatus, MoreKey } from '../../../config/profileEditorModel';
import type { SshKeyCheck } from '../../../hooks/useSshKeyCheck';

export type EditorPart = 'network' | 'user' | 'access' | 'region' | 'device' | 'secrets';
/** Areas that can be left unapplied, keeping the image's defaults */
export type DefaultsPart = 'network' | 'region' | 'access';

export interface EditorGroupProps {
  model: EditorForm;
  status: GroupStatus;
  /** Prefix for the ids of this editor's inputs, so a fix link can focus one */
  uid: string;
  patch: <K extends EditorPart>(key: K, value: Partial<EditorForm[K]>) => void;
  set: (value: Partial<EditorForm>) => void;
  keepDefaults: (key: DefaultsPart, byPointer: boolean) => void;
}

export interface FoldingGroupProps extends EditorGroupProps {
  more: boolean;
  toggleMore: (key: MoreKey) => void;
}

export interface AccessGroupProps extends FoldingGroupProps {
  keyCheck: SshKeyCheck;
}

export const fieldId = (uid: string, field: string) => `${uid}-${field}`;
