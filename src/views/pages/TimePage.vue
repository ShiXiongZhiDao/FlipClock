<script setup lang="ts">
import { useTheme } from "../../composables/useTheme";
import { t } from "../../lib/i18n";
import { themeLabel } from "../../lib/theme/labels";
import type { DigitLayout, TimeFormat } from "../../types/settings";

const { current: currentTheme, profile } = useTheme();

function setFormat(format: TimeFormat) {
  profile.value.format = format;
}

function setLayout(layout: DigitLayout) {
  profile.value.digitLayout = layout;
}
</script>

<template>
  <div class="time-page">
    <section class="section">
      <div class="section-head">
        <span class="section-title">{{ t("time.format") }}</span>
        <span class="section-note">{{ t("scope.currentTheme", { name: themeLabel(currentTheme) }) }}</span>
      </div>
      <div class="segmented narrow">
        <button
          type="button"
          :class="{ selected: profile.format === '24h' }"
          @click="setFormat('24h')"
        >
          {{ t("time.format24") }}
        </button>
        <button
          type="button"
          :class="{ selected: profile.format === '12h' }"
          @click="setFormat('12h')"
        >
          {{ t("time.format12") }}
        </button>
      </div>
    </section>

    <section class="section">
      <div class="section-title">{{ t("time.displayContent") }}</div>
      <div class="lines">
        <div class="row">
          <div class="row-copy">
            <span class="row-name">{{ t("time.showDate") }}</span>
            <span class="row-hint">{{ t("time.showDateHint") }}</span>
          </div>
          <button
            type="button"
            class="switch"
            role="switch"
            :aria-checked="profile.showDate"
            :class="{ on: profile.showDate }"
            @click="profile.showDate = !profile.showDate"
          >
            <i></i>
          </button>
        </div>

        <div class="row">
          <div class="row-copy">
            <span class="row-name">{{ t("time.showSeconds") }}</span>
          </div>
          <button
            type="button"
            class="switch"
            role="switch"
            :aria-checked="profile.showSeconds"
            :class="{ on: profile.showSeconds }"
            @click="profile.showSeconds = !profile.showSeconds"
          >
            <i></i>
          </button>
        </div>

        <div class="row">
          <div class="row-copy">
            <span class="row-name">{{ t("time.showColon") }}</span>
          </div>
          <button
            type="button"
            class="switch"
            role="switch"
            :aria-checked="profile.showColon"
            :class="{ on: profile.showColon }"
            @click="profile.showColon = !profile.showColon"
          >
            <i></i>
          </button>
        </div>
      </div>
    </section>

    <section class="section">
      <div class="section-title">{{ t("time.digitLayout") }}</div>
      <div class="layouts">
        <button
          type="button"
          class="layout-choice"
          :class="{ active: profile.digitLayout === 'per-digit' }"
          @click="setLayout('per-digit')"
        >
          <span class="layout-visual">
            <span class="mini-card">1</span>
            <span class="mini-card">0</span>
            <span class="colon">:</span>
            <span class="mini-card">2</span>
            <span class="mini-card">4</span>
          </span>
          <span class="layout-name">{{ t("time.layoutPerDigit") }}</span>
          <span class="layout-desc">{{ t("time.layoutPerDigitDesc") }}</span>
        </button>

        <button
          type="button"
          class="layout-choice"
          :class="{ active: profile.digitLayout === 'per-pair' }"
          @click="setLayout('per-pair')"
        >
          <span class="layout-visual">
            <span class="mini-card wide">10</span>
            <span class="mini-card wide">24</span>
          </span>
          <span class="layout-name">{{ t("time.layoutPerPair") }}</span>
          <span class="layout-desc">{{ t("time.layoutPerPairDesc") }}</span>
        </button>
      </div>
    </section>
  </div>
</template>

<style scoped>
.time-page {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.section-note {
  font-size: 9.5px;
  color: var(--ui-text-muted);
  max-width: 60%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* 两个选项不需要铺满整行；铺满会让人以为还有第三个选项要选。 */
.segmented.narrow {
  align-self: flex-start;
  /* 不设固定宽度：容器按按钮内容收缩，后面才不会多出一截空灰底。
     早先写死 208/264px 是按"别铺满整行"来的，但 align-self:flex-start
     本身就已经贴合内容，宽度反而成了多余的灰。 */
  width: max-content;
}

.layouts {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 9px;
}

.layout-choice {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 10px 11px 9px;
  border: 1px solid var(--ui-divider);
  border-radius: 4px;
  background: var(--ui-surface);
  text-align: left;
  color: var(--ui-text-secondary);
  transition: border-color 0.12s ease, box-shadow 0.12s ease;
}

.layout-choice:hover {
  border-color: #bdbab3;
}

.layout-choice.active {
  border-color: var(--ui-accent);
  box-shadow: inset 0 0 0 1px var(--ui-accent);
}

.layout-visual {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 3px;
  height: 46px;
  margin-bottom: 6px;
}

.mini-card {
  display: grid;
  place-items: center;
  width: 22px;
  height: 34px;
  border-radius: 3px;
  background: #37383c;
  color: #f6f6f4;
  font-family: var(--font-digit);
  font-size: 16px;
  line-height: 1;
  position: relative;
  overflow: hidden;
}

.mini-card::after {
  content: "";
  position: absolute;
  left: 0;
  right: 0;
  top: 50%;
  height: 1px;
  background: rgba(0, 0, 0, 0.55);
}

.mini-card.wide {
  width: 40px;
  font-size: 18px;
}

.colon {
  font-size: 15px;
  line-height: 1;
  color: var(--ui-text-muted);
  margin: 0 1px;
}

.layout-name {
  font-size: 11px;
  font-weight: 600;
  color: var(--ui-text);
}

.layout-desc {
  font-size: 9.5px;
  color: var(--ui-text-muted);
}
</style>
