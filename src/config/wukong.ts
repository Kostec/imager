// SPDX-License-Identifier: GPL-2.0-or-later

import type { BoardInfo } from '../types';

/** The board the app opens with; twin of the built-in catalog in src-tauri/src/catalog */
export const WUKONG_BOARD: BoardInfo = {
  slug: 'wukongpi',
  name: 'Wukong Pi Zero',
  vendor: 'wukong',
  vendor_name: 'Wukong',
  support_tier: 'standard',
  image_count: 0,
  has_desktop: false,
  promoted: false,
  soc: 'H2+',
  architecture: 'armhf',
};
