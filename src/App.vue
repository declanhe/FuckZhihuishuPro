<script setup lang="ts">
import { onMounted, ref } from "vue";
import { api } from "./api";
import Login from "./components/Login.vue";
import Home from "./components/Home.vue";
import "./style.css";

const page = ref<"loading" | "login" | "home">("loading");
const restoredTip = ref(false);

onMounted(async () => {
  // 恢复黑暗模式偏好
  if (localStorage.getItem("fzhs-dark") === "1") {
    document.documentElement.classList.add("dark");
  }
  try {
    // 尝试用本地 cookies 恢复登录态（短时间免登录）
    const restored = await api.restoreLogin();
    const state = await api.getLoginState();
    if (state.logged_in) {
      page.value = "home";
      if (restored) restoredTip.value = true;
      return;
    }
  } catch {
    /* ignore */
  }
  page.value = "login";
});

function onLogout() {
  page.value = "login";
}
</script>

<template>
  <Login v-if="page === 'login'" @done="page = 'home'" />
  <Home v-else-if="page === 'home'" @logout="onLogout" />
  <div v-else class="boot">正在启动…</div>
</template>

<style scoped>
.boot {
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--text-sub);
  font-size: 15px;
}
</style>
