<template>
  <Teleport to="body">
    <Transition name="dlg">
      <div v-if="dialogState.open" class="dlg-mask" @mousedown.self="cancel">
        <div class="dlg-box" role="alertdialog">
          <div class="dlg-icon">!</div>
          <p class="dlg-msg">{{ dialogState.message }}</p>
          <div class="dlg-btns">
            <button
              v-if="dialogState.mode === 'confirm'"
              class="btn-ghost"
              @click="cancel"
            >
              {{ t("dlg.cancel") }}
            </button>
            <button
              ref="okBtn"
              :class="dialogState.danger ? 'btn-danger' : 'btn-primary'"
              @click="ok"
            >
              {{ t("dlg.ok") }}
            </button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref, watch } from "vue";
import { dialogState, settleDialog } from "../dialog";
import { t } from "../i18n";

const okBtn = ref<HTMLButtonElement | null>(null);

function ok() {
  settleDialog(true);
}

function cancel() {
  settleDialog(false);
}

function onKey(e: KeyboardEvent) {
  if (!dialogState.open) return;
  if (e.key === "Escape") {
    e.preventDefault();
    cancel();
  } else if (e.key === "Enter") {
    e.preventDefault();
    ok();
  }
}

watch(
  () => dialogState.open,
  (v) => {
    if (v) {
      window.addEventListener("keydown", onKey);
      nextTick(() => okBtn.value?.focus());
    } else {
      window.removeEventListener("keydown", onKey);
    }
  }
);

onBeforeUnmount(() => window.removeEventListener("keydown", onKey));
</script>

<style scoped>
.dlg-mask {
  position: fixed;
  inset: 0;
  z-index: 1000;
  background: rgba(0, 0, 0, 0.42);
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 24px;
}

/* 圆角矩形卡片：颜色走主题变量，亮/暗模式自动适配 */
.dlg-box {
  background: var(--card);
  border-radius: 16px;
  box-shadow: 0 12px 40px rgba(0, 0, 0, 0.28);
  padding: 28px 28px 24px;
  width: 100%;
  max-width: 440px;
}

/* 红色惊叹号徽标 */
.dlg-icon {
  width: 46px;
  height: 46px;
  border-radius: 50%;
  background: var(--danger);
  color: #fff;
  font-size: 26px;
  font-weight: 800;
  display: flex;
  align-items: center;
  justify-content: center;
  margin: 0 auto 16px;
  box-shadow: 0 0 0 6px rgba(229, 72, 77, 0.14);
  user-select: none;
}

.dlg-msg {
  font-size: 16px;
  line-height: 1.7;
  color: var(--text);
  text-align: center;
  word-break: break-word;
  white-space: pre-wrap;
}

/* 两个按钮等大（各占一半宽度），单按钮时整行 */
.dlg-btns {
  display: flex;
  gap: 12px;
  margin-top: 22px;
}

.dlg-btns button {
  flex: 1;
  min-width: 0;
  font-size: 15px;
  padding: 11px 0;
}

/* 进出场：淡入 + 轻微缩放 */
.dlg-enter-active,
.dlg-leave-active {
  transition: opacity 0.16s ease;
}

.dlg-enter-active .dlg-box,
.dlg-leave-active .dlg-box {
  transition: transform 0.16s ease;
}

.dlg-enter-from,
.dlg-leave-to {
  opacity: 0;
}

.dlg-enter-from .dlg-box {
  transform: scale(0.94);
}

.dlg-leave-to .dlg-box {
  transform: scale(0.96);
}
</style>
