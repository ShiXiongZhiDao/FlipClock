import { computed } from "vue";

import { useTheme } from "./useTheme";
import {
  CLOCK_FIT_SCALE,
  CLOCK_SCALE_MAX,
  CLOCK_SCALE_MIN,
  CLOCK_SCALE_STEP,
} from "../types/settings";

/**
 * 占屏比例：把 clockScale 换算成「相对铺满」的倍数。
 *
 * CLOCK_FIT_SCALE 是「行宽刚好等于 16:9 屏宽」的缩放值，所以这个比值就是
 * 这块钟占屏宽的百分比。左栏预览与主题卡都按它画——同一个主题在卡片里、
 * 在左栏里、在屏保上必须是同一个大小，否则用户没法在卡片里挑主题。
 */
export function previewScaleOf(clockScale: number): number {
  return clockScale / CLOCK_FIT_SCALE;
}

/**
 * 时钟缩放的界面值与左栏预览的缩放系数。
 * 配置存在当前主题的钟面配置里（0.5–4.5），滑杆用百分比整数。
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

  /**
   * 预览卡里的缩放系数 = **真实的占屏比例**。
   *
   * 旧版这里是一段手调阻尼（0.72–1.27），理由是「真实占比会把栏宽撑爆」。
   * 那个理由建立在一个算错的 FIT 上（当时算成 1.234，实际 2.41）：225%
   * 其实只占屏宽 93%，根本不会撑爆；代价却是预览永远看不到裁切，而屏保
   * 那一侧会裁——2026-11-20 用户报的就是这件事。
   */
  const panelScale = computed(() => previewScaleOf(profile.value.clockScale));

  return { scalePercent, panelScale, SCALE_MIN, SCALE_MAX, SCALE_STEP, FIT_PERCENT };
}
