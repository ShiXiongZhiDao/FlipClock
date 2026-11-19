import { computed } from "vue";

import { useSettings } from "./useSettings";
import {
  CLOCK_FIT_SCALE,
  CLOCK_SCALE_MAX,
  CLOCK_SCALE_MIN,
  CLOCK_SCALE_STEP,
} from "../types/settings";

/**
 * 时钟缩放的界面值与预览标定，三个涉及钟面的页面共用一套。
 * 配置里存 0.5–4.5 的比例，滑杆用百分比整数。
 */
export function useClockScale() {
  const { settings } = useSettings();

  const scalePercent = computed({
    get: () => Math.round(settings.appearance.clockScale * 100),
    set: (value: number) => {
      settings.appearance.clockScale = value / 100;
    },
  });

  const SCALE_MIN = Math.round(CLOCK_SCALE_MIN * 100);
  const SCALE_MAX = Math.round(CLOCK_SCALE_MAX * 100);
  const SCALE_STEP = Math.round(CLOCK_SCALE_STEP * 100);

  /**
   * 让预览卡的钟面占比等于 stage 在 16:9 屏上的占比（实测标定，见 ADR 0007 的 2026-11-19 修订）。
   * 225% 时两边同为 87.6%；超过标定值后卡片两侧同样被裁，让用户提前看到全屏效果。
   */
  const previewScale = computed(() => settings.appearance.clockScale / CLOCK_FIT_SCALE);

  return { scalePercent, previewScale, SCALE_MIN, SCALE_MAX, SCALE_STEP };
}
