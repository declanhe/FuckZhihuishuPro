<template>
  <div class="home">
    <!-- 左侧边栏 -->
    <aside class="sidebar">
      <div class="brand">
        <div class="mark">知</div>
        <b>FZHS Pro</b>
      </div>

      <nav class="side-nav">
        <button
          v-for="t in tabs"
          :key="t.key"
          :class="{ active: tab === t.key }"
          @click="tab = t.key"
        >
          <span class="nav-icon">{{ t.icon }}</span>
          {{ t.label }}
          <span
            v-if="t.key === 'run' && runningCount > 0"
            class="badge"
            >{{ runningCount }}</span
          >
        </button>
      </nav>

      <div class="sidebar-footer">
        <button class="side-action" @click="stopAll" v-if="runningCount > 0">
          ⏹ 停止全部 ({{ runningCount }})
        </button>
        <button class="side-action logout" @click="doLogout">↩ 退出登录</button>
      </div>
    </aside>

    <!-- 右侧内容区 -->
    <main class="content">
    <section v-if="tab === 'courses'" class="page">
      <div class="toolbar">
        <button class="btn-ghost" @click="loadCourses" :disabled="loadingCourses">
          {{ loadingCourses ? "加载中…" : "刷新课程列表" }}
        </button>
        <label class="pick-all">
          <input type="checkbox" :checked="allPicked" @change="toggleAll" />
          全选
        </label>
        <button
          class="btn-primary"
          @click="startPicked"
          :disabled="pickedIds.size === 0"
        >
          开始刷课（{{ pickedIds.size }} 门）
        </button>
      </div>

      <p v-if="coursesError" class="error-tip">{{ coursesError }}</p>

      <div class="course-group" v-if="courses.zhidao.length">
        <h3>共享学分课（知到）</h3>
        <div
          v-for="c in courses.zhidao"
          :key="c.id"
          class="course-item card"
          :class="{ running: runningIds.has(c.id) }"
        >
          <label>
            <input
              type="checkbox"
              :checked="pickedIds.has(c.id)"
              @change="togglePick(c.id)"
            />
            <span class="course-name">{{ c.name }}</span>
          </label>
          <span class="course-tag" v-if="runningIds.has(c.id)">刷课中</span>
        </div>
      </div>

      <div class="course-group" v-if="courses.hike.length">
        <h3>校内学分课</h3>
        <div
          v-for="c in courses.hike"
          :key="c.id"
          class="course-item card"
          :class="{ running: runningIds.has(c.id) }"
        >
          <label>
            <input
              type="checkbox"
              :checked="pickedIds.has(c.id)"
              @change="togglePick(c.id)"
            />
            <span class="course-name">{{ c.name }}</span>
          </label>
          <span class="course-tag" v-if="runningIds.has(c.id)">刷课中</span>
        </div>
      </div>

      <div v-if="!loadingCourses && courses.zhidao.length === 0 && courses.hike.length === 0" class="empty">
        <p>没有查到课程 😢</p>
        <p class="sub">可能原因：账号下没有课程 / 登录已失效 / 网络异常，可点上方按钮刷新。</p>
      </div>
    </section>

    <!-- 运行状态 -->
    <section v-else-if="tab === 'run'" class="page">
      <div v-if="runs.size === 0" class="empty">
        <p>还没有任务在跑</p>
        <p class="sub">去「我的课程」勾选课程，点「开始刷课」即可。关掉窗口也会在后台继续刷。</p>
      </div>

      <div v-for="r in runList" :key="r.id" class="run-card card">
        <div class="run-head">
          <b>{{ r.name }}</b>
          <button class="btn-danger" @click="stopOne(r.id)" v-if="!r.done">
            停止
          </button>
          <span class="course-tag done-tag" v-else>{{ r.doneTag || "已完成" }}</span>
        </div>

        <div v-if="r.captcha" class="captcha-box">
          <b>⚠️ 账号被智慧树临时限制，刷课已暂停（不是软件坏了）</b>
          <p>连续刷课被服务器怀疑了，人工验证一次就能恢复：</p>
          <p class="captcha-steps">
            ① 先等 30~60 分钟，期间别反复重试（越试限制越严）<br />
            ② 用浏览器登录智慧树网页版，手动播放任意视频 1~2 分钟<br />
            ③ 回到这里，点左侧底部「退出登录」→ 重新扫码登录<br />
            ④ 先只挂一门课，等它跑完再挂下一门
          </p>
          <button class="btn-ghost" @click="restartRun(r)">
            我已完成验证，重新开始刷课
          </button>
        </div>

        <div class="run-now" v-if="!r.done">
          <template v-if="r.video">
            <div class="video-line">
              <span class="video-name" :title="r.video.name">
                {{ r.chapter ? r.chapter + " / " : "" }}{{ r.lesson ? r.lesson + " / " : "" }}{{ r.video.name }}
              </span>
              <span class="video-state" v-if="r.video.quiz">答题中…</span>
              <span class="video-state" v-else-if="r.video.paused">模拟暂停</span>
            </div>
            <div class="progress">
              <div
                class="progress-fill"
                :style="{ width: progressPct(r.video) + '%' }"
              ></div>
            </div>
            <div class="progress-text">
              {{ fmtTime(r.video.played) }} / {{ fmtTime(r.video.total) }}（{{ progressPct(r.video).toFixed(1) }}%）
            </div>
          </template>
          <p v-else class="run-log-main">{{ r.chapter || "准备中…" }}</p>
        </div>

        <div class="run-logs" ref="logBoxes">
          <p v-for="(l, i) in r.logs.slice(-80)" :key="i" :class="'log-' + l.level">
            {{ l.msg }}
          </p>
        </div>
      </div>
    </section>

    <!-- 设置 -->
    <section v-else class="page settings">
      <div class="settings-grid">
        <div class="setting-item card">
          <label>
            播放倍速
            <select v-model.number="cfg.speed">
              <option :value="0">平台最高（推荐，共享课 1.5x / 校内课 1.25x）</option>
              <option :value="1">1.0x</option>
              <option :value="1.25">1.25x</option>
              <option :value="1.5">1.5x</option>
              <option :value="1.8">1.8x（最高）</option>
            </select>
          </label>
          <p class="setting-tip">默认就是最稳的选择；倍速越高刷得越快，被智慧树盯上的概率也越大。</p>
        </div>

        <div class="setting-item card">
          <label>
            完成阈值：{{ Math.round(cfg.end_threshold * 100) }}%
            <input
              type="range"
              min="0.85"
              max="1"
              step="0.01"
              v-model.number="cfg.end_threshold"
            />
          </label>
          <p class="setting-tip">
            播放进度达到该百分比即视为学完，默认 100% 刷满整个视频。
          </p>
        </div>

        <div class="setting-item card">
          <label>
            单门课程时限（分钟，0 = 不限）
            <input type="text" v-model.number="cfg.limit_minutes" />
          </label>
          <p class="setting-tip">
            建议参考课程学习习惯中的建议学习时长。
          </p>
        </div>

        <div class="setting-item card">
          <div class="toggle-row">
            <div>
              <b>最小化到托盘</b>
              <p class="setting-tip">
                关闭窗口时最小化到托盘，后台继续刷课（推荐开启）。托盘右键可退出。
              </p>
            </div>
            <button
              class="toggle"
              :class="{ on: cfg.minimize_to_tray }"
              @click="cfg.minimize_to_tray = !cfg.minimize_to_tray"
              role="switch"
              :aria-checked="cfg.minimize_to_tray"
            >
              <span class="knob"></span>
            </button>
          </div>
        </div>

        <div class="setting-item card">
          <div class="toggle-row">
            <div>
              <b>黑暗模式</b>
              <p class="setting-tip">深色界面，夜间刷课更护眼，立即生效。</p>
            </div>
            <button
              class="toggle"
              :class="{ on: darkMode }"
              @click="darkMode = !darkMode; applyDark()"
              role="switch"
              :aria-checked="darkMode"
            >
              <span class="knob"></span>
            </button>
          </div>
        </div>
      </div>

      <button class="btn-primary save-btn" @click="saveCfg">保存设置</button>
      <p class="save-ok" v-if="saveOk">✓ 已保存</p>
    </section>
    </main>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, reactive } from "vue";
import {
  api,
  onCourseEvent,
  type AppConfig,
  type CourseItem,
} from "../api";

const emit = defineEmits<{ logout: [] }>();

const tabs = [
  { key: "courses", label: "我的课程", icon: "📚" },
  { key: "run", label: "刷课进度", icon: "⚡" },
  { key: "settings", label: "设置", icon: "⚙" },
] as const;

const tab = ref<"courses" | "run" | "settings">("courses");

// ---------- 课程列表 ----------
const courses = reactive<{ zhidao: CourseItem[]; hike: CourseItem[] }>({
  zhidao: [],
  hike: [],
});
const loadingCourses = ref(false);
const coursesError = ref("");
const pickedIds = ref(new Set<string>());
const runningIds = ref(new Set<string>());

const allPicked = computed(() => {
  const all = [...courses.zhidao, ...courses.hike];
  return all.length > 0 && all.every((c) => pickedIds.value.has(c.id));
});

async function loadCourses() {
  loadingCourses.value = true;
  coursesError.value = "";
  try {
    const r = await api.getCourses();
    courses.zhidao = r.zhidao;
    courses.hike = r.hike;
  } catch (e) {
    coursesError.value = "课程加载失败：" + String(e);
  } finally {
    loadingCourses.value = false;
  }
}

function togglePick(id: string) {
  const s = new Set(pickedIds.value);
  if (s.has(id)) s.delete(id);
  else s.add(id);
  pickedIds.value = s;
}

function toggleAll() {
  const s = new Set<string>();
  if (!allPicked.value) {
    for (const c of [...courses.zhidao, ...courses.hike]) s.add(c.id);
  }
  pickedIds.value = s;
}

async function startPicked() {
  const picked = [...courses.zhidao, ...courses.hike].filter((c) =>
    pickedIds.value.has(c.id)
  );
  try {
    const failed = await api.startCourses(picked);
    for (const c of picked) {
      runningIds.value.add(c.id);
      ensureRun(c.id, c.name);
    }
    pickedIds.value = new Set();
    tab.value = "run";
    if (failed.length > 0) {
      alert(failed.length + " 门课程启动失败（可能已在刷课中）");
    }
  } catch (e) {
    alert("启动失败：" + String(e));
  }
}

// ---------- 运行状态 ----------
interface RunVideo {
  video_id: string;
  name: string;
  played: number;
  total: number;
  paused: boolean;
  quiz: boolean;
}
interface RunState {
  id: string;
  name: string;
  chapter: string;
  lesson: string;
  video: RunVideo | null;
  logs: { level: string; msg: string }[];
  done: boolean;
  /** 终态标签：已完成 / 已停止 / 待验证 / 已达时限 */
  doneTag: string;
  /** 账号被智慧树临时限制（-12），卡片上展示恢复引导 */
  captcha: boolean;
}

const runs = reactive(new Map<string, RunState>());
const runList = computed(() => [...runs.values()]);
const runningCount = computed(
  () => [...runs.values()].filter((r) => !r.done).length
);

function ensureRun(id: string, name: string): RunState {
  const existing = runs.get(id);
  if (existing) {
    // 重新启动同一门课：重置状态（上次运行可能已标记 done，且旧日志会误导）
    if (existing.done) {
      existing.chapter = "";
      existing.lesson = "";
      existing.video = null;
      existing.logs = [];
      existing.done = false;
      existing.doneTag = "";
      existing.captcha = false;
    }
    return existing;
  }
  runs.set(id, {
    id,
    name,
    chapter: "",
    lesson: "",
    video: null,
    logs: [],
    done: false,
    doneTag: "",
    captcha: false,
  });
  return runs.get(id)!;
}

function pushLog(r: RunState, level: string, msg: string) {
  const t = new Date().toLocaleTimeString("zh-CN", { hour12: false });
  r.logs.push({ level, msg: `[${t}] ${msg}` });
  if (r.logs.length > 300) r.logs.splice(0, r.logs.length - 300);
}

function progressPct(v: RunVideo): number {
  if (!v.total) return 0;
  return Math.min(100, (v.played / v.total) * 100);
}

function fmtTime(s: number): string {
  const sec = Math.max(0, Math.floor(s));
  const h = Math.floor(sec / 3600);
  const m = Math.floor((sec % 3600) / 60);
  const ss = sec % 60;
  return `${h}:${String(m).padStart(2, "0")}:${String(ss).padStart(2, "0")}`;
}

let unlisten: (() => void) | null = null;

onMounted(async () => {
  unlisten = await onCourseEvent(({ courseId, courseName, event }) => {
    const r = ensureRun(courseId, courseName);
    switch (event.kind) {
      case "log":
        pushLog(r, event.level || "info", event.msg || "");
        break;
      case "chapter":
        r.chapter = event.name || "";
        pushLog(r, "info", `章节：${r.chapter}`);
        break;
      case "lesson":
        r.lesson = event.name || "";
        break;
      case "video_start":
        r.video = {
          video_id: event.video_id || "",
          name: event.name || "",
          played: event.played || 0,
          total: event.total || 0,
          paused: false,
          quiz: false,
        };
        pushLog(r, "info", `开始观看：${event.name}`);
        break;
      case "video_progress":
        // 后端 VideoProgress 事件只有 video_id 没有 name，必须按 video_id 匹配
        if (r.video && r.video.video_id === event.video_id) {
          r.video.played = event.played || 0;
          r.video.total = event.total || 0;
          r.video.paused = !!event.paused;
          r.video.quiz = !!event.quiz;
        }
        break;
      case "video_done":
        pushLog(r, "info", `看完：${event.name}`);
        r.video = null;
        break;
      case "quiz_answered":
        pushLog(r, "ok", `已自动答题：${event.question || ""}`);
        break;
      case "skipped":
        pushLog(r, "info", `跳过 ${event.name}：${event.reason}`);
        break;
      case "limit_reached":
        pushLog(r, "warn", "达到单课程时限，本课程停止");
        r.done = true;
        r.doneTag = "已达时限";
        runningIds.value.delete(courseId);
        break;
      case "captcha_required":
        r.done = true;
        r.captcha = true;
        r.doneTag = "待验证";
        pushLog(r, "warn", "账号被智慧树临时限制，任务已暂停（见上方提示）");
        runningIds.value.delete(courseId);
        break;
      case "done":
        pushLog(r, "ok", event.msg || "课程完成");
        r.done = true;
        if ((event.msg || "").includes("限制")) {
          // -12 在任务开头就被拦下时不会发 captcha_required，靠这里兜底展示引导
          r.captcha = true;
          r.doneTag = "待验证";
        } else {
          r.doneTag = (event.msg || "").includes("停止") ? "已停止" : "已完成";
        }
        runningIds.value.delete(courseId);
        break;
      case "failed":
        pushLog(r, "error", event.msg || "");
        // 关键失败（账号被限制）时任务实际已停止，显示终态避免进度条凝固
        if ((event.msg || "").includes("限制") || (event.msg || "").includes("验证码")) {
          r.done = true;
          r.captcha = true;
          r.doneTag = "待验证";
          runningIds.value.delete(courseId);
        }
        break;
    }
  });

  // 恢复运行中的任务列表（页面刷新时）
  try {
    const ids = await api.getRunning();
    for (const id of ids) runningIds.value.add(id);
  } catch {
    /* ignore */
  }
  await loadCourses();
  await loadCfg();
});

onBeforeUnmount(() => {
  if (unlisten) unlisten();
});

async function stopOne(id: string) {
  await api.stopCourse(id);
  runningIds.value.delete(id);
  const r = runs.get(id);
  if (r) {
    r.done = true;
    r.doneTag = "已停止";
    pushLog(r, "warn", "已手动停止");
  }
}

async function stopAll() {
  await api.stopAll();
  for (const r of runs.values()) {
    if (!r.done) {
      r.done = true;
      r.doneTag = "已停止";
      pushLog(r, "warn", "已手动停止");
    }
  }
  runningIds.value.clear();
}

/** 完成人工验证后，从卡片上直接重新开始这门课 */
async function restartRun(r: RunState) {
  r.captcha = false;
  try {
    const failed = await api.startCourses([{ id: r.id, name: r.name }]);
    if (failed.length === 0) {
      runningIds.value.add(r.id);
      ensureRun(r.id, r.name);
    } else {
      alert("启动失败（该课程可能已在刷课中）");
    }
  } catch (e) {
    alert("启动失败：" + String(e));
  }
}

async function doLogout() {
  if (runningCount.value > 0 && !confirm("还有任务在刷课，退出登录将全部停止，确定？"))
    return;
  await api.logout();
  emit("logout");
}

// ---------- 设置 ----------
const cfg = ref<AppConfig>({
  speed: 0,
  end_threshold: 1.0,
  limit_minutes: 0,
  minimize_to_tray: true,
});
const saveOk = ref(false);

// 黑暗模式（UI 偏好存 localStorage，不走后端配置）
const darkMode = ref(localStorage.getItem("fzhs-dark") === "1");

function applyDark() {
  document.documentElement.classList.toggle("dark", darkMode.value);
  localStorage.setItem("fzhs-dark", darkMode.value ? "1" : "0");
}

async function loadCfg() {
  cfg.value = await api.getConfig();
}

async function saveCfg() {
  await api.saveConfig(cfg.value);
  saveOk.value = true;
  setTimeout(() => (saveOk.value = false), 2000);
}
</script>

<style scoped>
.home {
  height: 100%;
  display: flex;
}

/* ---------- 左侧边栏 ---------- */
.sidebar {
  width: 200px;
  flex-shrink: 0;
  background: var(--card);
  display: flex;
  flex-direction: column;
  padding: 16px 12px;
  box-shadow: 1px 0 3px var(--card-shadow);
  overflow: hidden;
}

.brand {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 4px 8px 16px;
  min-width: 0;
}

.brand b {
  font-size: 13px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.mark {
  position: relative;
  overflow: hidden;
  width: 36px;
  height: 36px;
  background: linear-gradient(135deg, var(--green), #0aa);
  border-radius: 10px;
  color: #fff;
  font-size: 18px;
  font-weight: 700;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

/* 与应用图标同款禁制斜杠（左上→右下） */
.mark::after {
  content: "";
  position: absolute;
  left: -25%;
  top: 50%;
  width: 150%;
  height: 5px;
  border-radius: 3px;
  background: #e0483c;
  box-shadow: 0 0 0 1.5px #fff;
  transform: translateY(-50%) rotate(45deg);
  pointer-events: none;
}

.side-nav {
  display: flex;
  flex-direction: column;
  gap: 4px;
  flex: 1;
}

.side-nav button {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 11px 14px;
  background: transparent;
  color: var(--text-sub);
  font-size: 14.5px;
  border-radius: 10px;
  text-align: left;
}

.side-nav button:hover {
  background: var(--tab-bg);
  color: var(--text);
}

.side-nav button.active {
  background: var(--green-light);
  color: var(--green-dark);
  font-weight: 600;
}

.nav-icon {
  font-size: 16px;
}

.badge {
  background: var(--green);
  color: #fff;
  border-radius: 10px;
  padding: 1px 8px;
  font-size: 12px;
  margin-left: auto;
}

.sidebar-footer {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding-top: 10px;
  border-top: 1px solid var(--progress-track);
}

.side-action {
  background: transparent;
  color: var(--text-sub);
  font-size: 13.5px;
  padding: 9px 14px;
  border-radius: 8px;
  text-align: left;
}

.side-action:hover {
  background: var(--tab-bg);
  color: var(--text);
}

.side-action.logout {
  color: var(--danger);
}

/* ---------- 内容区 ---------- */
.content {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.page {
  flex: 1;
  overflow-y: auto;
  padding: 18px 22px;
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.toolbar {
  display: flex;
  align-items: center;
  gap: 14px;
}

.pick-all {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 14px;
  color: var(--text-sub);
}

.course-group h3 {
  font-size: 15px;
  margin: 6px 0 10px;
  color: var(--text);
}

.course-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 13px 18px;
  margin-bottom: 8px;
}

.course-item label {
  display: flex;
  align-items: center;
  gap: 12px;
  cursor: pointer;
  font-size: 15px;
}

.course-item.running {
  opacity: 0.75;
}

.course-name {
  max-width: 560px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.course-tag {
  color: var(--green-dark);
  background: var(--green-light);
  font-size: 12px;
  padding: 3px 10px;
  border-radius: 10px;
}

.done-tag {
  color: var(--text-sub);
  background: var(--progress-track);
}

.error-tip {
  color: var(--danger);
  font-size: 13px;
}

.empty {
  text-align: center;
  color: var(--text-sub);
  padding: 60px 0;
  font-size: 16px;
}

.empty .sub {
  font-size: 13px;
  margin-top: 8px;
}

/* 运行卡片 */
.run-card {
  padding: 16px 20px;
}

.run-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 10px;
}

.run-head b {
  font-size: 15px;
}

.captcha-box {
  margin: 4px 0 12px;
  padding: 14px 16px;
  border-radius: 10px;
  background: rgba(240, 173, 40, 0.12);
  border: 1px solid rgba(240, 173, 40, 0.5);
  font-size: 13px;
  line-height: 1.7;
}

.captcha-box > b {
  display: block;
  margin-bottom: 4px;
  color: #c07f00;
}

html.dark .captcha-box > b {
  color: #ffc861;
}

.captcha-box p {
  color: var(--text-sub);
}

.captcha-box .btn-ghost {
  margin-top: 8px;
}

.run-log-main {
  color: var(--text-sub);
  font-size: 13px;
}

.video-line {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 8px;
}

.video-name {
  font-size: 14px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 640px;
}

.video-state {
  color: var(--warn);
  font-size: 13px;
  flex-shrink: 0;
}

.progress {
  height: 8px;
  background: var(--progress-track);
  border-radius: 4px;
  overflow: hidden;
}

.progress-fill {
  height: 100%;
  background: linear-gradient(90deg, var(--green), #0aa);
  transition: width 0.6s linear;
}

.progress-text {
  color: var(--text-sub);
  font-size: 12px;
  margin-top: 6px;
}

.run-logs {
  margin-top: 10px;
  max-height: 150px;
  overflow-y: auto;
  background: var(--log-bg);
  border-radius: 8px;
  padding: 8px 12px;
  font-size: 12px;
  line-height: 1.8;
}

.log-ok {
  color: var(--green-dark);
}

.log-warn {
  color: var(--warn);
}

.log-error {
  color: var(--danger);
}

/* 设置：宽窗口自动双列，卡片统一等高 */
.settings-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(340px, 1fr));
  grid-auto-rows: 1fr;
  gap: 14px;
}

.setting-item {
  padding: 18px 22px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.setting-item label {
  display: block;
  font-size: 15px;
  font-weight: 600;
}

.setting-item select,
.setting-item input[type="text"] {
  margin-top: 10px;
}

.setting-item input[type="range"] {
  width: 100%;
  margin-top: 12px;
  accent-color: var(--green);
}

/* toggle 行（左文右开关） */
.toggle-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 14px;
}

.toggle-row b {
  font-size: 15px;
}

.toggle-row .setting-tip {
  margin-top: 4px;
}

/* iOS 风格 toggle 开关 */
.toggle {
  width: 46px;
  height: 26px;
  border-radius: 13px;
  background: var(--scroll-thumb);
  position: relative;
  flex-shrink: 0;
  transition: background 0.2s ease;
  padding: 0;
}

.toggle .knob {
  position: absolute;
  top: 3px;
  left: 3px;
  width: 20px;
  height: 20px;
  border-radius: 50%;
  background: #fff;
  box-shadow: 0 1px 3px var(--card-shadow);
  transition: left 0.2s ease;
}

.toggle.on {
  background: var(--green);
}

.toggle.on .knob {
  left: 23px;
}

.setting-tip {
  color: var(--text-sub);
  font-size: 12.5px;
  margin-top: 8px;
  line-height: 1.6;
}

.save-btn {
  align-self: flex-start;
  width: auto;
  margin: 0;
}

.save-ok {
  color: var(--green-dark);
  font-size: 13px;
}
</style>
