<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from "vue";

const props = withDefaults(
  defineProps<{
    title: string;
    confirmLabel: string;
    cancelLabel: string;
    tone?: "primary" | "danger";
    confirmDisabled?: boolean;
    submitOnEnter?: boolean;
  }>(),
  { tone: "primary", confirmDisabled: false, submitOnEnter: false },
);

const emit = defineEmits<{
  (event: "cancel"): void;
  (event: "confirm"): void;
}>();

const dialogEl = ref<HTMLElement | null>(null);
let returnFocus: HTMLElement | null = null;

function focusableItems(): HTMLElement[] {
  const root = dialogEl.value;
  if (!root) return [];
  return [
    ...root.querySelectorAll<HTMLElement>(
      "button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), a[href], [tabindex]:not([tabindex='-1'])",
    ),
  ];
}

function onKeydown(event: KeyboardEvent): void {
  // 输入法用 Esc 取消候选、用 Enter 上屏时，不能动弹窗
  if (event.isComposing) return;

  if (event.key === "Escape") {
    event.preventDefault();
    emit("cancel");
    return;
  }

  if (event.key === "Tab") {
    const items = focusableItems();
    if (!items.length) {
      event.preventDefault();
      dialogEl.value?.focus({ preventScroll: true });
      return;
    }
    const active = document.activeElement as HTMLElement | null;
    const index = active ? items.indexOf(active) : -1;
    if (event.shiftKey && index <= 0) {
      event.preventDefault();
      items[items.length - 1].focus();
    } else if (!event.shiftKey && (index < 0 || index === items.length - 1)) {
      event.preventDefault();
      items[0].focus();
    }
    return;
  }

  if (event.key === "Enter" && props.submitOnEnter) {
    // 焦点在按钮上时交给按钮自己的 Enter 行为，避免取消键变成提交
    const target = event.target instanceof HTMLElement ? event.target : null;
    if (target?.closest("button, a, [role='button']")) return;
    if (props.confirmDisabled) return;
    event.preventDefault();
    emit("confirm");
  }
}

onMounted(() => {
  returnFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
  window.addEventListener("keydown", onKeydown);
  // 确认弹窗的焦点落容器而不是按钮：WebView2 会把程序化 focus 算成
  // :focus-visible，落按钮会顶着焦点环出现（ADR-0028）。输入弹窗由页面
  // 在 nextTick 里把焦点移到输入框，晚于这里。
  dialogEl.value?.focus({ preventScroll: true });
});

onBeforeUnmount(() => {
  window.removeEventListener("keydown", onKeydown);
  if (returnFocus?.isConnected) returnFocus.focus({ preventScroll: true });
});
</script>

<template>
  <div class="dialog-backdrop" @click.self="emit('cancel')">
    <div
      ref="dialogEl"
      class="dialog"
      role="dialog"
      aria-modal="true"
      :aria-label="title"
      tabindex="-1"
    >
      <h2 class="dialog-title">{{ title }}</h2>
      <slot />
      <footer class="dialog-actions">
        <button type="button" class="secondary" @click="emit('cancel')">
          {{ cancelLabel }}
        </button>
        <button
          type="button"
          :class="tone === 'danger' ? 'danger' : 'primary'"
          :disabled="confirmDisabled"
          @click="emit('confirm')"
        >
          {{ confirmLabel }}
        </button>
      </footer>
    </div>
  </div>
</template>

<style scoped>
.dialog-backdrop {
  position: fixed;
  inset: 0;
  z-index: 650;
  display: grid;
  place-items: center;
  padding: 20px;
  background: var(--ui-scrim);
  animation: dialog-scrim-in 0.12s ease-out;
}

.dialog {
  width: min(348px, calc(100% - 8px));
  padding: 15px 17px 14px;
  border: 1px solid var(--ui-divider-strong);
  border-radius: 6px;
  background: var(--ui-surface);
  box-shadow: var(--ui-shadow-dialog);
  animation: dialog-in 0.12s ease-out;
}

.dialog:focus {
  outline: none;
}

.dialog-title {
  margin: 0;
  font-size: 13.5px;
  font-weight: 600;
  letter-spacing: -0.005em;
  color: var(--ui-text);
}

.dialog-actions {
  display: flex;
  justify-content: flex-end;
  gap: 6px;
  margin-top: 13px;
}

.dialog-actions .danger {
  padding: 6px 11px;
  border: 1px solid var(--ui-danger);
  border-radius: var(--radius-card);
  background: var(--ui-danger);
  color: var(--ui-danger-ink);
  font-size: 10.5px;
  font-weight: 600;
  white-space: nowrap;
  transition: background 0.14s ease, border-color 0.14s ease;
}

.dialog-actions .danger:hover:not(:disabled) {
  border-color: var(--ui-danger-hover);
  background: var(--ui-danger-hover);
}

.dialog-actions .danger:disabled {
  opacity: 0.45;
  cursor: default;
}

@keyframes dialog-scrim-in {
  from {
    opacity: 0;
  }
}

@keyframes dialog-in {
  from {
    opacity: 0;
    transform: scale(0.985);
  }
}

@media (prefers-reduced-motion: reduce) {
  .dialog-backdrop,
  .dialog {
    animation: none;
  }
}
</style>