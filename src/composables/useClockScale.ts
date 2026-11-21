import { computed } from "vue";

import { useTheme } from "./useTheme";
import {
  CLOCK_FIT_SCALE,
  CLOCK_SCALE_MAX,
  CLOCK_SCALE_MIN,
  CLOCK_SCALE_STEP,
} from "../types/settings";

/**
 * 时钟缩放的界面值与滑杆参照点。
 * 配置存在当前主题的钟面配置里（0.5–4.5），滑杆用百分比整数。
 *
 * 预览不再在这里换算：左栏、主题卡与全屏共用 FlipClock 的同一套几何
 * （stage 用 vmin、panel 用 cqmin，再乘同一个 clockScale）。
 */
export function useClockScale() {
  const { profile } = useTheme();

  const scalePercent = computed({
    get: () => Math.round(profile.value.clockScale * 100),
    set: (value: number) => {
      profile.value.clockScale = value / 100;
    },
  });

  const SCALE_MIN = Math.round(CLOCK_SCALE_MIN * 100);
  const SCALE_MAX = Math.round(CLOCK_SCALE_MAX * 100);
  const SCALE_STEP = Math.round(CLOCK_SCALE_STEP * 100);

  const FIT_PERCENT = Math.round(CLOCK_FIT_SCALE * 100);

  return { scalePercent, SCALE_MIN, SCALE_MAX, SCALE_STEP, FIT_PERCENT };
}
