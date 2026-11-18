<template>
  <div class="login-page">
    <div class="login-card card">
      <div class="logo">
        <div class="logo-mark">知</div>
        <h1>FZHS Pro</h1>
        <p class="slogan">把网课丢给它，你去吃点好的</p>
      </div>

      <div class="tabs">
        <button
          :class="{ active: mode === 'qr' }"
          @click="switchMode('qr')"
        >
          扫码登录（推荐）
        </button>
        <button
          v-if="!isMac"
          :class="{ active: mode === 'pwd' }"
          @click="switchMode('pwd')"
        >
          浏览器登录
        </button>
      </div>

      <!-- 扫码登录 -->
      <div v-if="mode === 'qr'" class="qr-panel">
        <div class="qr-box">
          <img
            v-if="qrImg"
            :src="qrImg"
            alt="登录二维码"
            :class="{ dim: qrScanned }"
          />
          <div v-else class="qr-loading">{{ qrError || "正在获取二维码…" }}</div>
          <div v-if="qrScanned" class="qr-overlay">✓ 已扫码<br />确认后正在登录…</div>
        </div>
        <p class="qr-hint">📱 打开<b>知到 APP</b>，扫一扫上面的二维码</p>
        <p class="hint warn-hint" v-if="qrError && qrImg">{{ qrError }}</p>
        <button class="btn-ghost" @click="startQr" :disabled="!qrImg && qrLoading">
          刷新二维码
        </button>
      </div>

      <!-- 浏览器登录：主窗口切换到智慧树登录页，账号密码/滑块/短信都在网页里完成 -->
      <div v-else class="pwd-panel">
        <p class="hint browser-hint">
          点击下方按钮后，本窗口会变成智慧树登录页面。<br />
          在页面里输入账号密码（需要滑块/短信验证也直接完成），<br />
          <b>登录成功后本程序会自动返回并进入主界面</b>。
        </p>
        <button class="btn-primary" @click="openBrowser" :disabled="browserOpening">
          {{ browserOpening ? "正在打开…" : "打开登录页面" }}
        </button>
        <p class="hint warn-hint" v-if="pwdError">{{ pwdError }}</p>
        <p class="hint">推荐使用「扫码登录」，无需输入账号密码更安全。</p>
      </div>
    </div>

    <p class="footer">
      仅供学习交流 · 请合理安排时间 · 本工具不上传任何数据到第三方
    </p>
  </div>
</template>

<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from "vue";
import { listen } from "@tauri-apps/api/event";
import { api, parseErr } from "../api";

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
    qrError.value = parseErr(e).message + "（点此重试）";
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
    }
    polling = false;
    pollTimer = window.setTimeout(pollQr, 800);
  } catch (e) {
    const err = parseErr(e);
    polling = false;
    if (err.kind === "qr_cancelled") {
      stopPoll();
      qrError.value = "已在手机上取消登录，请点下方按钮刷新二维码重试";
      return;
    }
    if (err.kind === "qr_expired") {
      // 过期自动刷新，并给出可见提示（避免看起来"没反应"）
      qrError.value = "二维码已过期，正在自动刷新…";
      await startQr();
      return;
    }
    // 网络/其他错误：提示原因，停留在当前页等待用户手动重试
    qrError.value = err.message + "（请检查网络后点下方按钮重试）";
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

onMounted(async () => {
  // 内嵌浏览器登录成功 -> 直接进入主界面
  const un = await listen("browser-login-success", () => {
    emit("done");
  });
  onBeforeUnmount(un);
});

onBeforeUnmount(stopPoll);
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
