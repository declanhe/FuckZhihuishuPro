<template>
  <div class="login-page">
    <div class="login-card card">
      <div class="logo">
        <div class="logo-mark">知</div>
        <h1>FZHS Pro</h1>
        <p class="slogan">{{ t("login.slogan") }}</p>
      </div>

      <div class="tabs">
        <button
          :class="{ active: mode === 'qr' }"
          @click="switchMode('qr')"
        >
          {{ t("login.tabQr") }}
        </button>
        <button
          v-if="!isMac"
          :class="{ active: mode === 'pwd' }"
          @click="switchMode('pwd')"
        >
          {{ t("login.tabBrowser") }}
        </button>
      </div>

      <!-- 扫码登录 -->
      <div v-if="mode === 'qr'" class="qr-panel">
        <div class="qr-box">
          <img
            v-if="qrImg"
            :src="qrImg"
            :alt="t('login.qrAlt')"
            :class="{ dim: qrScanned }"
          />
          <div v-else class="qr-loading">{{ qrError || t("login.qrFetching") }}</div>
          <div v-if="qrScanned" class="qr-overlay" v-html="t('login.qrScanned')"></div>
        </div>
        <p class="qr-hint" v-html="t('login.qrHint')"></p>
        <p class="hint warn-hint" v-if="qrError && qrImg">{{ qrError }}</p>
        <button class="btn-ghost" @click="startQr" :disabled="!qrImg && qrLoading">
          {{ t("login.qrRefresh") }}
        </button>
      </div>

      <!-- 浏览器登录：主窗口切换到智慧树登录页，账号密码/滑块/短信都在网页里完成 -->
      <div v-else class="pwd-panel">
        <p class="hint browser-hint" v-html="t('login.browserHint')"></p>
        <button class="btn-primary" @click="openBrowser" :disabled="browserOpening">
          {{ browserOpening ? t("login.opening") : t("login.openBrowser") }}
        </button>
        <p class="hint warn-hint" v-if="pwdError">{{ pwdError }}</p>
        <p class="hint">{{ t("login.browserTip") }}</p>
      </div>
    </div>

    <p class="footer">
      {{ t("login.footer") }}
    </p>
  </div>
</template>

<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from "vue";
import { listen } from "@tauri-apps/api/event";
import { api, parseErr } from "../api";
import { t } from "../i18n";

const emit = defineEmits<{ done: [] }>();

// 浏览器登录依赖 WebView2 CookieManager，仅 Windows 提供（macOS 只有扫码登录）
const isMac = /Macintosh|Mac OS X/i.test(navigator.userAgent);

const mode = ref<"qr" | "pwd">("qr");
const qrImg = ref("");
const qrScanned = ref(false);
const qrError = ref("");
const qrLoading = ref(false);
const browserOpening = ref(false);
const pwdError = ref("");

let pollTimer: number | null = null;
let currentToken = "";
let polling = false;

async function startQr() {
  qrImg.value = "";
  qrScanned.value = false;
  qrError.value = "";
  qrLoading.value = true;
  try {
    const r = await api.qrLoginStart();
    currentToken = r.qrToken;
    qrImg.value = `data:image/png;base64,${r.img}`;
    pollQr();
  } catch (e) {
    qrError.value = parseErr(e).message + t("login.retry");
  } finally {
    qrLoading.value = false;
  }
}

async function pollQr() {
  if (polling || !currentToken) return;
  polling = true;
  try {
    const r = await api.qrLoginPoll(currentToken);
    if (r.status === "scanned") {
      qrScanned.value = true;
    } else if (r.status === "success") {
      stopPoll();
      emit("done");
      return;
    } else if (r.status === "expired") {
      // 过期：立即自动刷新（后端现在明确返回 expired，不再静默当 waiting）
      qrError.value = t("login.expired");
      polling = false;
      await startQr();
      return;
    }
    polling = false;
    pollTimer = window.setTimeout(pollQr, 800);
  } catch (e) {
    const err = parseErr(e);
    polling = false;
    if (err.kind === "qr_cancelled") {
      stopPoll();
      qrError.value = t("login.cancelled");
      return;
    }
    if (err.kind === "qr_expired") {
      // 过期自动刷新，并给出可见提示（避免看起来"没反应"）
      qrError.value = t("login.expired");
      await startQr();
      return;
    }
    // 网络/其他错误：提示原因，停留在当前页等待用户手动重试
    qrError.value = err.message + t("login.netRetry");
  }
}

function stopPoll() {
  if (pollTimer) {
    clearTimeout(pollTimer);
    pollTimer = null;
  }
  currentToken = "";
}

function switchMode(m: "qr" | "pwd") {
  mode.value = m;
  if (m === "qr") startQr();
  else stopPoll();
}

/** 打开内嵌浏览器登录窗口；成功后后端会发 browser-login-success 事件 */
async function openBrowser() {
  browserOpening.value = true;
  pwdError.value = "";
  try {
    await api.openBrowserLogin();
  } catch (e) {
    pwdError.value = parseErr(e).message;
  } finally {
    browserOpening.value = false;
  }
}

// 【监听器泄漏修复】同步注册清理钩子（await 之后注册无效），
// 并用 unmounted 标志处理异步注册期间组件已卸载的竞态。
let unlistenBrowser: (() => void) | null = null;
let unmounted = false;

onMounted(async () => {
  const un = await listen("browser-login-success", () => {
    emit("done");
  });
  if (unmounted) {
    un(); // 组件已卸载，立即释放监听器
  } else {
    unlistenBrowser = un;
  }
});

onBeforeUnmount(() => {
  unmounted = true;
  stopPoll();
  unlistenBrowser?.();
  unlistenBrowser = null;
});
startQr();
</script>

<style scoped>
.login-page {
  height: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 20px;
}

.login-card {
  width: 420px;
  padding: 36px 40px;
  display: flex;
  flex-direction: column;
  align-items: center;
}

.logo {
  text-align: center;
  margin-bottom: 22px;
}

.logo-mark {
  position: relative;
  overflow: hidden;
  width: 64px;
  height: 64px;
  margin: 0 auto 10px;
  background: linear-gradient(135deg, var(--green), #0aa);
  border-radius: 16px;
  color: #fff;
  font-size: 32px;
  font-weight: 700;
  display: flex;
  align-items: center;
  justify-content: center;
}

/* 与应用图标同款禁制斜杠 */
.logo-mark::after {
  content: "";
  position: absolute;
  left: -25%;
  top: 50%;
  width: 150%;
  height: 7px;
  border-radius: 4px;
  background: #e0483c;
  box-shadow: 0 0 0 2px #fff;
  transform: translateY(-50%) rotate(45deg);
  pointer-events: none;
}

.logo h1 {
  font-size: 22px;
  letter-spacing: 0.5px;
}

.slogan {
  color: var(--text-sub);
  font-size: 12px;
  margin-top: 6px;
}

.tabs {
  display: flex;
  background: var(--tab-bg);
  border-radius: 10px;
  padding: 4px;
  margin-bottom: 22px;
  width: 100%;
}

.tabs button {
  flex: 1;
  padding: 9px 0;
  background: transparent;
  border-radius: 8px;
  font-size: 15px;
  color: var(--text-sub);
}

.tabs button.active {
  background: var(--tab-active-bg);
  color: var(--green-dark);
  font-weight: 600;
  box-shadow: 0 1px 3px var(--tab-active-shadow);
}

.qr-panel {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 14px;
}

.qr-box {
  position: relative;
  width: 200px;
  height: 200px;
  border: 1px solid var(--input-border);
  border-radius: 10px;
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
}

.qr-box img {
  width: 100%;
  height: 100%;
  object-fit: contain;
}

.qr-box img.dim {
  filter: blur(3px) opacity(0.5);
}

.qr-loading {
  color: var(--text-sub);
  font-size: 13px;
  padding: 10px;
  text-align: center;
  cursor: pointer;
}

.qr-overlay {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  background: var(--tab-active-bg);
  color: var(--green-dark);
  font-size: 16px;
  font-weight: 600;
  line-height: 1.8;
  text-align: center;
}

.hint {
  color: var(--text-sub);
  font-size: 12.5px;
  text-align: center;
  line-height: 1.7;
}

/* 扫码提示：绿色药丸，一眼看到该用什么扫 */
.qr-hint {
  margin: 4px 0 2px;
  padding: 9px 16px;
  border-radius: 999px;
  background: rgba(46, 196, 160, 0.14);
  color: var(--green-dark);
  font-size: 13.5px;
  font-weight: 600;
  text-align: center;
}

html.dark .qr-hint {
  color: var(--green);
}

.warn-hint {
  color: var(--danger);
}

.pwd-panel {
  width: 100%;
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.footer {
  color: #9aa7ae;
  font-size: 12px;
}
</style>
