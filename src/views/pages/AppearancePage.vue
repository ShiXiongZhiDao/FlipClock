<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { computed } from "vue";

import FlipClock from "../../components/clock/FlipClock.vue";
import FontPicker from "../../components/theme/FontPicker.vue";
import { useClock } from "../../composables/useClock";
import { useClockScale } from "../../composables/useClockScale";
import { useSettings } from "../../composables/useSettings";
import { useStudioLauncher } from "../../composables/useStudioLauncher";
import { useToast } from "../../composables/useToast";
import { useTheme } from "../../composables/useTheme";
import { isTauri } from "../../lib/env";
import { ensureFontLoaded, forgetFont, setFontLibrary, uniqueFontName } from "../../lib/fonts";
import { errorText, locale, t } from "../../lib/i18n";
import { rangeFill } from "../../lib/rangeFill";
import { forgetThemeAssets, themeAssetUrl } from "../../lib/theme/assets";
import { blankTheme, cloneTheme, isBuiltinThemeId } from "../../lib/theme/presets";
import { profileFor, themeIdsUsingFont } from "../../lib/theme/profile";
import { themeLabel } from "../../lib/theme/labels";
import { uniqueThemeName, fitThemeStem } from "../../lib/theme/names";
import { swatchStyle } from "../../lib/theme/swatch";
import {
  BRIGHTNESS_MAX,
  BRIGHTNESS_MIN,
  CUSTOM_FONT_LIMIT,
  CUSTOM_THEME_LIMIT,
  type CustomFont,
  type ThemeSpec,
} from "../../types/settings";

const { settings } = useSettings();
/** 主题预览共用同一个实时时刻：既走时，又保持配色可比。 */
const { now } = useClock();
const { all: allThemes, current: currentTheme, profile } = useTheme();
const { launch } = useStudioLauncher();
const { scalePercent, SCALE_MIN, SCALE_MAX, SCALE_STEP, FIT_PERCENT } = useClockScale();

/** 页面内的轻提示。设置页有一套全局 toast，但那是 SettingsView 自己的，
 *  这里只需要一条「已达上限 / 已删除」的短反馈。 */
const { toast, notify } = useToast(2600);

const canImportFonts = isTauri();

/** 主题卡画的是「这个主题自己的钟面」，不能借用当前主题的配置。 */
function cardProfile(theme: ThemeSpec) {
  return profileFor(settings, theme.id);
}

const BRIGHTNESS_MIN_PERCENT = Math.round(BRIGHTNESS_MIN * 100);
const BRIGHTNESS_MAX_PERCENT = Math.round(BRIGHTNESS_MAX * 100);

/** 亮度存在当前主题的钟面配置里；滑杆用整数百分比，存的是 0–1。 */
const brightnessPercent = computed({
  get: () => Math.round(profile.value.brightness * 100),
  set: (value: number) => {
    profile.value.brightness = value / 100;
  },
});

/* --------------------------------- 主题管理 -------------------------------- */

function atLimit(): boolean {
  if (settings.appearance.customThemes.length < CUSTOM_THEME_LIMIT) return false;
  notify(t("appearance.limitReached", { limit: CUSTOM_THEME_LIMIT }));
  return true;
}

/** 新主题 id：`custom-<uuid>`（ADR-0020 D12）。 */
function newThemeId(): string {
  return `custom-${crypto.randomUUID()}`;
}

/** 主题名要去重并压进 16 字符，规则与 Rust 侧一致，见 lib/theme/names.ts。 */
function uniqueName(base: string): string {
  return uniqueThemeName(
    base,
    settings.appearance.customThemes.map((theme) => theme.name),
  );
}

/* --------------------------------- 字体库 -------------------------------- */

async function removeFontFile(font: Pick<CustomFont, "id" | "file">): Promise<void> {
  await invoke("remove_font", { fontId: font.id, filename: font.file }).catch(
    (error) => {
      console.warn("remove font file failed", font.id, error);
    },
  );
}

function fontLimitReached(): boolean {
  if (settings.appearance.fonts.length < CUSTOM_FONT_LIMIT) return false;
  notify(t("appearance.fontLimit", { limit: CUSTOM_FONT_LIMIT }));
  return true;
}

async function importOneFont(path: string): Promise<void> {
  const id = `custom-${crypto.randomUUID()}`;
  try {
    const imported = await invoke<CustomFont>("import_font", {
      fontId: id,
      sourcePath: path,
    });
    const duplicate = settings.appearance.fonts.find(
      (font) => font.hash === imported.hash,
    );
    if (duplicate) {
      await removeFontFile(imported);
      profile.value.font = duplicate.id;
      notify(t("appearance.fontDuplicate", { name: duplicate.name }));
      return;
    }

    const entry: CustomFont = {
      ...imported,
      name: uniqueFontName(
        imported.name,
        settings.appearance.fonts.map((font) => font.name),
        t("appearance.fontDefaultName"),
      ),
    };
    // 先放进库再加载：字体管理器从设置里按 id 找文件；加载失败整条撤回。
    settings.appearance.fonts.push(entry);
    setFontLibrary(settings.appearance.fonts);
    if (!(await ensureFontLoaded(entry))) {
      settings.appearance.fonts = settings.appearance.fonts.filter(
        (font) => font.id !== entry.id,
      );
      forgetFont(entry.id);
      await removeFontFile(entry);
      notify(t("appearance.fontLoadFailed"));
      return;
    }
    profile.value.font = entry.id;
    notify(t("appearance.fontImported", { name: entry.name }));
  } catch (error) {
    notify(errorText(error));
  }
}

async function importFonts(): Promise<void> {
  if (!canImportFonts || fontLimitReached()) return;

  let selected: string | string[] | null;
  try {
    selected = await open({
      multiple: true,
      filters: [
        {
          name: t("appearance.fontFilter"),
          extensions: ["ttf", "otf", "woff", "woff2"],
        },
      ],
    });
  } catch (error) {
    notify(errorText(error));
    return;
  }
  if (!selected) return;

  for (const path of Array.isArray(selected) ? selected : [selected]) {
    if (fontLimitReached()) break;
    await importOneFont(path);
  }
}

function renameFont(font: CustomFont): void {
  const next = window.prompt(t("appearance.fontRenamePrompt"), font.name);
  if (next === null) return;
  const trimmed = next.trim();
  if (!trimmed) {
    notify(t("appearance.fontRenameEmpty"));
    return;
  }
  font.name = uniqueFontName(
    trimmed,
    settings.appearance.fonts
      .filter((item) => item.id !== font.id)
      .map((item) => item.name),
    t("appearance.fontDefaultName"),
  );
}

async function removeFont(font: CustomFont): Promise<void> {
  const affectedIds = themeIdsUsingFont(settings, font.id);
  const affected = affectedIds
    .map((themeId) => allThemes.value.find((theme) => theme.id === themeId))
    .filter((theme): theme is ThemeSpec => !!theme)
    .map((theme) => themeLabel(theme));
  const separator = locale.value === "zh" ? "、" : ", ";
  const message = affected.length
    ? t("appearance.fontDeleteConfirm", {
        name: font.name,
        count: affected.length,
        themes: affected.join(separator),
      })
    : t("appearance.fontDeleteConfirmUnused", { name: font.name });
  if (!window.confirm(message)) return;

  for (const item of Object.values(settings.appearance.profiles)) {
    if (item.font === font.id) item.font = "bebas";
  }
  settings.appearance.fonts = settings.appearance.fonts.filter(
    (item) => item.id !== font.id,
  );
  forgetFont(font.id);
  await removeFontFile(font);
  notify(t("appearance.fontRemoved", { name: font.name }));
}

function openFailure(error: unknown): void {
  notify(t("appearance.fullscreenFailed"));
  console.warn("open fullscreen setup failed", error);
}

/**
 * 双击主题卡上的钟面直接进入全屏配置。单击已把该主题设为当前主题，
 * 这里再显式设一次，避免依赖 click 与 dblclick 的触发顺序；launch("clock")
 * 会先 flush 再开窗，全屏配置里调的就是这个主题的钟面配置（ADR-0022）。
 */
function openClockSetup(theme: ThemeSpec): void {
  settings.appearance.theme = theme.id;
  void launch("clock").catch(openFailure);
}

function startNew(): void {
  if (atLimit()) return;
  const id = newThemeId();
  void launch("theme", {
    theme: blankTheme(id, uniqueName(t("appearance.myTheme"))),
    editingThemeId: id,
    newThemeIds: [id],
  }).catch(openFailure);
}

function startClone(source: ThemeSpec): void {
  if (atLimit()) return;
  const id = newThemeId();
  // 后缀占几个字符由字典决定（中文「 副本」3 个、英文「 copy」5 个），
  // 先把原名字让出这段长度，否则截断会把后缀切掉半个。
  const reserve = [...t("appearance.copySuffix", { name: "" })].length;
  const name = t("appearance.copySuffix", { name: fitThemeStem(themeLabel(source), reserve) });
  void launch("theme", {
    theme: cloneTheme(source, id, uniqueName(name)),
    // 复制连钟面配置一起复制（ADR-0022）。
    profileFrom: source.id,
    editingThemeId: id,
    newThemeIds: [id],
  }).catch(openFailure);
}

function startEdit(theme: ThemeSpec): void {
  void launch("theme", { editingThemeId: theme.id }).catch(openFailure);
}

async function removeTheme(theme: ThemeSpec): Promise<void> {
  if (!window.confirm(t("appearance.deleteConfirm", { name: themeLabel(theme) }))) return;
  const index = settings.appearance.customThemes.findIndex((item) => item.id === theme.id);
  if (index >= 0) settings.appearance.customThemes.splice(index, 1);
  // 配置与主题成对存在：删主题必须连带删它的钟面配置（ADR-0022）。
  delete settings.appearance.profiles[theme.id];
  if (settings.appearance.theme === theme.id) {
    settings.appearance.theme = "classic-dark";
    notify(t("appearance.deletedInUse"));
  }
  forgetThemeAssets(theme.id);
  await invoke("delete_theme_dir", { themeId: theme.id }).catch(() => {});
}
</script>

<template>
  <div class="appearance">
    <div v-if="toast" class="toast">{{ toast }}</div>

    <section class="section">
      <div class="section-head">
        <span class="section-title">{{ t("appearance.themeTitle") }}</span>
      </div>

      <div class="theme-grid">
        <div
          v-for="theme in allThemes"
          :key="theme.id"
          class="theme-cell"
          :class="{ active: settings.appearance.theme === theme.id }"
        >
          <button
            type="button"
            class="theme-pick theme-sample"
            :class="{ 'is-active': settings.appearance.theme === theme.id }"
            :style="swatchStyle(theme)"
            :title="t('studio.dblclickHint')"
            @click="settings.appearance.theme = theme.id"
            @dblclick.prevent="openClockSetup(theme)"
          >
            <span class="theme-stage stage theme-sample-stage">
              <FlipClock
                :now="now"
                :format="cardProfile(theme).format"
                :show-date="cardProfile(theme).showDate"
                :show-seconds="cardProfile(theme).showSeconds"
                :theme="theme"
                :background-url="themeAssetUrl(theme.id, theme.background.image)"
                :digit-url="themeAssetUrl(theme.id, theme.digit.image)"
                :layout="cardProfile(theme).digitLayout"
                :show-colon="cardProfile(theme).showColon"
                :font="cardProfile(theme).font"
                :brightness="cardProfile(theme).brightness"
                :scale="cardProfile(theme).clockScale"
                variant="panel"
              />
            </span>
            <span class="theme-name theme-sample-name">{{ themeLabel(theme) }}</span>
          </button>

          <!-- 三个图标压在名字行右侧：底色是主题色块，所以每个按钮自带一层深色
               半透明底，任何主题（近黑、亮金）上都保证白色图标清晰。
               位置沿用原来的右下角，名字用 padding-right 让位。 -->
          <div class="theme-actions">
            <button
              type="button"
              class="icon"
              :title="t('appearance.clone')"
              :aria-label="t('appearance.clone')"
              @click="startClone(theme)"
            >
              <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4">
                <rect x="5.5" y="5.5" width="8" height="8" rx="1.6" />
                <path d="M10.5 3.5v-1a1 1 0 0 0-1-1h-7a1 1 0 0 0-1 1v7a1 1 0 0 0 1 1h1" />
              </svg>
            </button>
            <button
              v-if="!isBuiltinThemeId(theme.id)"
              type="button"
              class="icon"
              :title="t('appearance.edit')"
              :aria-label="t('appearance.edit')"
              @click="startEdit(theme)"
            >
              <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4">
                <path d="M11.2 2.4l2.4 2.4-8 8H3.2v-2.4z" />
              </svg>
            </button>
            <button
              v-if="!isBuiltinThemeId(theme.id)"
              type="button"
              class="icon danger"
              :title="t('appearance.remove')"
              :aria-label="t('appearance.remove')"
              @click="removeTheme(theme)"
            >
              <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4">
                <path d="M3 4.5h10M6.5 4.5V3h3v1.5M4.5 4.5l.6 8h5.8l.6-8" />
              </svg>
            </button>
          </div>
        </div>

        <button
          type="button"
          class="theme-cell new-theme"
          :title="t('appearance.newTheme')"
          @click="startNew"
        >
          <span class="new-stage"><span class="plus">+</span></span>
          <span class="theme-name theme-sample-name">{{ t("appearance.newTheme") }}</span>
        </button>
      </div>
    </section>

    <section class="section">
      <!-- 字体、亮度、缩放都存在当前主题的钟面配置里（ADR-0022）。
           不说这一句，用户会以为调的是全局。 -->
      <p class="scope-note">
        {{ t("scope.currentTheme", { name: themeLabel(currentTheme) }) }}
      </p>

      <div class="columns">
        <div class="col">
          <div class="font-head">
            <div class="section-title">{{ t("appearance.fontLabel") }}</div>
            <button
              v-if="canImportFonts"
              type="button"
              class="font-import"
              @click="importFonts"
            >
              {{ t("appearance.fontImport") }}
            </button>
          </div>
          <FontPicker
            :model-value="profile.font"
            :fonts="settings.appearance.fonts"
            manage
            @update:model-value="profile.font = $event"
            @load-failed="notify(t('appearance.fontLoadFailed'))"
            @rename="renameFont"
            @remove="removeFont"
          />
        </div>

        <div class="col">
          <div class="section-title">{{ t("appearance.effects") }}</div>

          <div class="range-row">
            <div class="range-head">
              <span>{{ t("appearance.brightness") }}</span>
              <span class="range-value">{{ brightnessPercent }}%</span>
            </div>
            <input
              v-model.number="brightnessPercent"
              class="range"
              type="range"
              :min="BRIGHTNESS_MIN_PERCENT"
              :max="BRIGHTNESS_MAX_PERCENT"
              step="5"
              :style="rangeFill(brightnessPercent, BRIGHTNESS_MIN_PERCENT, BRIGHTNESS_MAX_PERCENT)"
              :aria-label="t('appearance.brightnessAria')"
            />
          </div>

          <div class="range-row">
            <div class="range-head">
              <span>
                {{ t("clock.scale") }}
                <!-- 「刚好铺满 16:9 屏」的标定值挂在标签上，而不是再加一行刻度：
                     多一行就把整页顶出可视区，值本身又只是参考。 -->
                <em class="fit-hint">{{ t("clock.scaleFit", { percent: FIT_PERCENT }) }}</em>
              </span>
              <span class="range-value">{{ scalePercent }}%</span>
            </div>
            <input
              v-model.number="scalePercent"
              class="range"
              type="range"
              :min="SCALE_MIN"
              :max="SCALE_MAX"
              :step="SCALE_STEP"
              :style="rangeFill(scalePercent, SCALE_MIN, SCALE_MAX)"
              :aria-label="t('clock.scaleAria')"
            />
          </div>
        </div>
      </div>
    </section>
  </div>
</template>

<style scoped>
/* 固定悬浮，别用 sticky：这一页自己不滚动（滚动的是外层 .page-scroll），
   sticky 会占位，弹一下就把整页内容往下推。 */
.toast {
  position: fixed;
  left: 50%;
  bottom: 30px;
  transform: translateX(-50%);
  z-index: 60;
  padding: 7px 13px;
  font-size: 11px;
  border-radius: 4px;
  background: var(--ui-toast-bg);
  color: var(--ui-toast-ink);
}

.appearance {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

/* 主题不是「卡片」，是样本：一块钟面 + 名字，外面不套框。
   套了框就变成卡表，五张卡并排会像商品列表。 */
.theme-grid {
  display: grid;
  /* 两列：样卡保持足够大，才能和左栏实时预览对上。 */
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 14px 12px;
  /* 左右留出选中环与悬停放大的余量：选中环 4px + 悬停放大 3.5%
     每边约 3.6px，贴着 .page-scroll 的裁剪线会削掉左/右边框；
     12px 在 125% DPI 与亚像素取整下也不会再贴边。 */
  padding: 0 12px;
}

.theme-cell {
  display: flex;
  flex-direction: column;
  min-width: 0;
  text-align: left;
  /* 给绝对定位的操作条当定位参考，也让悬停时的 z-index 生效。 */
  position: relative;
  /* 悬停放大作用在整张卡上——若只放大里面的 .theme-pick，
     绝对定位的操作条不会跟着，按钮会和卡片边缘错开。 */
  transition: transform 0.16s ease;
}

.theme-cell:hover {
  transform: scale(1.035);
  /* 放大后压在邻卡之上，别被后一张卡盖住半个角。 */
  z-index: 3;
}

/* 样卡的形状、边框、名字排版都在 ui.css 的 .theme-sample（与全屏配置共用），
   这里只留这一处网格特有的东西：行高与悬停反馈。 */
.theme-pick {
  /* .section button 把行高钉成 20px 是为了分割线落在整数像素上；
     这里内部是纯文字堆叠，20px 会把每格撑高、把整页顶出可视区。 */
  line-height: 1.35;
}

/* 悬停放大时加一层浅投影，让「浮起」有落点。 */
.theme-cell:hover .theme-sample {
  box-shadow: var(--ui-shadow-float);
}

/* 选中卡悬停：外阴影不能冲掉那圈墨环，两层合并。 */
.theme-cell.active:hover .theme-sample {
  box-shadow:
    0 0 0 2px var(--ui-surface),
    0 0 0 4px var(--ui-accent),
    var(--ui-shadow-float);
}

/* 新建卡没有 .theme-pick（它的框就是 .new-stage），沿用同一套悬停反馈。 */
.theme-cell.new-theme:hover .new-stage {
  border-color: var(--ui-accent);
  box-shadow: var(--ui-shadow-float);
}

/* 标题与右下角的操作条同一行：右侧留出三个按钮的位置，文字才不会压到
   「删除」上。新建卡没有操作条，不留这段空白。 */
.theme-cell:not(.new-theme) .theme-name {
  padding-right: 88px;
}

/* 选中卡只加粗，**不改颜色**：名字的颜色是主题的数字色，
   换成界面的墨色会在近黑主题卡上直接看不见。 */
.theme-cell.active .theme-name {
  font-weight: 600;
}

/* 操作条落在卡片右下、与标题同一行：.theme-pick 是 button，不能再嵌 button，
   所以用兄弟节点 + 绝对定位贴在它内部，边框与双击热区都不受影响。
   默认透明、悬停才出现，位置始终占着，卡片高度不变。 */
.theme-actions {
  position: absolute;
  right: 8px;
  bottom: 6px;
  z-index: 1;
  display: flex;
  gap: 4px;
  opacity: 0;
  /* 透明时不吃鼠标：否则卡片底部那一行藏着三个可点区域，
     会抢走「双击钟面进全屏配置」的点击。 */
  pointer-events: none;
  transition: opacity 0.12s ease;
}

.theme-cell:hover .theme-actions {
  opacity: 1;
  pointer-events: auto;
}

/* 键盘导航要能看到操作条，但不能用 :focus-within：鼠标点过主题后
   .theme-pick 会一直留着焦点，操作条就常显了。:focus-visible 只在键盘
   Tab 过来时匹配，鼠标点击不算——「选中但不悬浮」时按钮保持隐藏。 */
.theme-cell:has(:focus-visible) .theme-actions {
  opacity: 1;
  pointer-events: auto;
}

/* 图标钮压在主题色块上（近黑卡、亮金卡都有），不能靠界面灰阶取色：
   深色半透明底 + **一圈 1px 亮边**——深色卡上靠亮边定形，亮色卡上靠深底定形，
   白色图标两边都清楚。22px 圆钮 + 4px 间距 ×3 = 74px，落在名字右侧留出的 88px 里。 */
.theme-actions .icon {
  display: grid;
  place-items: center;
  width: 22px;
  height: 22px;
  padding: 0;
  border: 0;
  border-radius: 50%;
  background: var(--ui-on-theme);
  box-shadow: inset 0 0 0 1px var(--ui-on-theme-ring);
  color: var(--ui-on-theme-ink);
  transition: background 0.12s ease;
}

.theme-actions .icon:hover {
  background: var(--ui-on-theme-hover);
  box-shadow: inset 0 0 0 1px var(--ui-on-theme-ring-hover);
}

.theme-actions .icon.danger:hover {
  background: var(--ui-danger);
}

.theme-actions svg {
  width: 13px;
  height: 13px;
  stroke-width: 1.5;
}

/* 新建：一块空样本，不是虚线卡片。虚线卡片是「占位符」的语汇，
   这里要的是「再来一块」的语汇。外框与普通卡片的 .theme-pick 同构
   （同样的 padding / 边框 / gap / 宽高比），尺寸才对得上。 */
/* 新建卡与主题卡同构：**整块料连名字行一起填**，中间是与主屏同比例的钟面位，
   只是料用界面底色、名字用灰的——它不指向任何主题，不该借某个主题的颜色。
   以前是「带边框的空盒子 + 伪元素撑高」，在一排色块里像个缺口。 */
.new-theme {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 6px;
  border: 0;
  border-radius: var(--radius-panel);
  /* 用第三层灰（--ui-surface-3）而不是第二层（--ui-surface-2）：后者在白底与抽屉的渐变底上
     几乎看不出来，一整块料就成了"浮着的两个字"。 */
  background: var(--ui-surface-3);
  text-align: left;
  /* 与 .theme-pick 同一条理由：.section button 的 20px 行高会把名字行撑高，
     整格就比旁边的主题卡高一截。 */
  line-height: 1.35;
  color: var(--ui-text-muted);
  transition: color 0.12s ease;
}

/* 悬停只压深文字，不改底色：再深一档没有对应的 token，
   而这块料的职责是"占位"，不该比旁边的主题卡更抢眼。 */
.new-theme:hover {
  color: var(--ui-text-secondary);
}


.new-theme .plus {
  font-size: 20px;
  line-height: 1;
  /* 加号本身没有笔画粗细可调，稍微压一点透明，免得比钟面还抢眼 */
  opacity: 0.6;
  transition: opacity 0.12s ease;
}

.new-theme:hover .plus {
  opacity: 1;
}

.columns {
  display: grid;
  grid-template-columns: minmax(0, 0.92fr) minmax(0, 1.08fr);
  gap: 26px;
  align-items: start;
}

.col {
  display: flex;
  flex-direction: column;
  gap: 9px;
  min-width: 0;
}

.font-head {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 8px;
}

.font-import {
  border: 0;
  padding: 0;
  background: none;
  color: var(--ui-accent);
  font: inherit;
  font-size: 10px;
  cursor: pointer;
}

.font-import:hover {
  text-decoration: underline;
}

.range-row {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding-top: 8px;
  border-top: 1px solid var(--ui-divider);
  font-size: 11px;
  color: var(--ui-text-secondary);
}

.col .range-row:first-of-type {
  padding-top: 0;
  border-top: 0;
}

.fit-hint {
  margin-left: 6px;
  font-style: normal;
  font-size: 9.5px;
  color: var(--ui-text-muted);
}

.scope-note {
  margin: -3px 0 0;
  font-size: 9.5px;
  color: var(--ui-text-muted);
}

@media (max-width: 1080px) {
  .columns {
    grid-template-columns: minmax(0, 1fr);
    gap: 15px;
  }
}
</style>
