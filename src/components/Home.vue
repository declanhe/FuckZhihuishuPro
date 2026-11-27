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
          v-for="tb in tabs"
          :key="tb.key"
          :class="{ active: tab === tb.key }"
          @click="tab = tb.key"
        >
          <span class="nav-icon">{{ tb.icon }}</span>
          {{ t(tb.labelKey) }}
          <span
            v-if="tb.key === 'run' && runningCount > 0"
            class="badge"
            >{{ runningCount }}</span
          >
        </button>
      </nav>

      <div class="sidebar-footer">
        <button class="side-action logout" @click="doLogout">
          {{ t("side.logout") }}
        </button>
      </div>
    </aside>

    <!-- 右侧内容区 -->
    <main class="content">
    <section v-if="tab === 'courses'" class="page">
      <div class="toolbar">
        <button class="btn-ghost" @click="loadCourses" :disabled="loadingCourses">
          {{ loadingCourses ? t("btn.refreshing") : t("btn.refresh") }}
        </button>
        <button
          class="btn-primary start-btn"
          @click="startPicked"
          :disabled="pickedIds.size === 0"
        >
          {{ t("btn.start", pickedIds.size) }}
        </button>
      </div>

      <p v-if="coursesError" class="error-tip">{{ coursesError }}</p>
      <p v-else-if="courseList.length" class="drag-tip">{{ t("dragTip") }}</p>

      <div ref="courseListBox" class="course-list">
        <template v-for="(c, i) in courseList" :key="c.id">
          <!-- 拖拽中：原位置显示占位框 -->
          <div
            v-if="dragId === c.id"
            class="drag-placeholder"
            :style="{ height: dragH + 'px' }"
          ></div>
          <div
            v-else
            class="course-item card"
            :class="{
              running: isCurrent(c.id),
              'is-queued': isQueued(c.id),
            }"
          >
            <label>
              <span class="course-no">{{ i + 1 }}</span>
              <input
                type="checkbox"
                :checked="pickedIds.has(c.id)"
                @change="togglePick(c.id)"
              />
              <span class="dot" :class="c.type === 'zhidao' ? 'dot-share' : 'dot-school'"></span>
              <span class="course-name" :title="c.name">{{ c.name }}</span>
              <span class="type-label">{{ typeLabel(c.type) }}</span>
            </label>
            <div class="course-right">
              <span class="course-tag" v-if="isCurrent(c.id)">{{ t("tag.running") }}</span>
              <span class="course-tag done-tag" v-else-if="isDone(c.id)">{{ t("tag.done") }}</span>
              <span class="course-tag queued-tag" v-else-if="isQueued(c.id)">{{ t("tag.queued") }}</span>
              <span
                class="drag-handle"
                :title="t('dragTip')"
                @mousedown.prevent="onHandleDown(c.id, $event)"
                >⠿</span
              >
            </div>
          </div>
        </template>
      </div>

      <!-- 拖拽时跟随鼠标的"幽灵"卡片：拿起放大、松手缩回并吸附到落点 -->
      <div
        v-if="dragId && dragCourse"
        class="drag-ghost card course-item"
        :class="{ dropping: dragDropping }"
        :style="ghostStyle"
      >
        <label>
          <span class="course-no">{{ dragIndex + 1 }}</span>
          <span class="dot" :class="dragCourse.type === 'zhidao' ? 'dot-share' : 'dot-school'"></span>
          <span class="course-name">{{ dragCourse.name }}</span>
          <span class="type-label">{{ typeLabel(dragCourse.type) }}</span>
        </label>
        <div class="course-right">
          <span class="drag-handle">⠿</span>
        </div>
      </div>

      <!-- 全选：列表末尾的最后一个框，复选框与条目对齐 -->
      <div v-if="courseList.length" class="course-item card pick-all-row">
        <label>
          <span class="course-no"></span>
          <input type="checkbox" :checked="allPicked" @change="toggleAll" />
          <span class="pick-all-text">{{ t("pickAll") }}</span>
        </label>
      </div>

      <div v-if="!loadingCourses && courseList.length === 0" class="empty">
        <p>{{ t("empty.courses.title") }}</p>
        <p class="sub">{{ t("empty.courses.sub") }}</p>
      </div>
    </section>

    <!-- 运行状态 -->
    <section v-else-if="tab === 'run'" class="page">
      <div class="run-toolbar" v-if="runRows.length > 0">
        <button class="btn-ghost" v-if="runningCount > 0" @click="togglePause">
          {{ queueState.paused ? t("btn.resume") : t("btn.pause") }}
        </button>
        <button class="btn-ghost clear-all" @click="clearAll">
          {{ t("btn.clearAll") }}
        </button>
        <span v-if="queueState.paused && runningCount > 0" class="paused-tip">{{ t("run.pausedTip") }}</span>
      </div>

      <p v-if="captchaNotice" class="queue-notice">{{ t("notice.captchaCleared") }}</p>

      <div v-if="runRows.length === 0" class="empty">
        <p>{{ t("run.empty.title") }}</p>
        <p class="sub">{{ t("run.empty.sub") }}</p>
      </div>

      <template v-for="(row, i) in runRows" :key="rowKey(row)">
        <h3 v-if="sectionTitle(row, i)" class="section-title">
          {{
            sectionTitle(row, i) === "run.section.queue"
              ? t("run.section.queue", queueState.pending.length)
              : t(sectionTitle(row, i)!)
          }}
        </h3>

        <!-- 排队中的课程 -->
        <div v-if="row.rowType === 'queued'" class="queue-item card">
          <span class="queue-no">{{ row.pos }}</span>
          <span class="queue-name" :title="row.course.name">{{ row.course.name }}</span>
          <button class="btn-ghost sm" @click="removeQueued(row.course.id)">
            {{ t("btn.remove") }}
          </button>
        </div>

        <!-- 正在刷 / 已结束共用卡片 -->
        <div
          v-else
          class="run-card card"
          :class="row.run.done ? 'is-done' : 'is-current'"
        >
          <div class="run-head">
            <b>{{ row.run.name }}</b>
            <button
              v-if="!row.run.done"
              class="btn-danger"
              @click="skipCurrent(row.run.id)"
            >
              {{ t("btn.skip") }}
            </button>
            <span class="course-tag done-tag" v-else-if="row.run.doneTag">{{
              t(row.run.doneTag)
            }}</span>
          </div>

          <div v-if="row.run.captcha" class="captcha-box">
            <b>{{ t("captcha.title") }}</b>
            <p>{{ t("captcha.intro") }}</p>
            <p class="captcha-steps">
              {{ t("captcha.s1") }}<br />
              {{ t("captcha.s2") }}<br />
              {{ t("captcha.s3") }}<br />
              {{ t("captcha.s4") }}
            </p>
            <button class="btn-ghost" @click="restartRun(row.run)">
              {{ t("captcha.retry") }}
            </button>
          </div>

          <div class="run-now" v-if="!row.run.done">
            <template v-if="row.run.video">
              <div class="video-line">
                <span class="video-name" :title="row.run.video.name">
                  {{ row.run.chapter ? row.run.chapter + " / " : "" }}{{ row.run.lesson ? row.run.lesson + " / " : "" }}{{ row.run.video.name }}
                </span>
                <span class="video-state" v-if="queueState.paused">{{ t("state.paused") }}</span>
                <span class="video-state" v-else-if="row.run.video.quiz">{{ t("state.quiz") }}</span>
                <span class="video-state" v-else-if="row.run.video.paused">{{ t("state.simPause") }}</span>
              </div>
              <div class="progress">
                <div
                  class="progress-fill"
                  :style="{ width: progressPct(row.run.video) + '%' }"
                ></div>
              </div>
              <div class="progress-text">
                {{ fmtTime(row.run.video.played) }} / {{ fmtTime(row.run.video.total) }} ({{ progressPct(row.run.video).toFixed(1) }}%)
              </div>
            </template>
            <p v-else class="run-log-main">{{ row.run.chapter || t("run.preparing") }}</p>
          </div>

          <div class="run-logs">
            <p
              v-for="(l, li) in row.run.logs.slice(-80)"
              :key="li"
              :class="'log-' + l.level"
            >
              {{ l.msg }}
            </p>
          </div>
        </div>
      </template>
    </section>

    <!-- 设置 -->
    <section v-else class="page settings">
      <div class="settings-grid">
        <div class="setting-item card">
          <label>
            {{ t("set.speed") }}
            <select v-model.number="cfg.speed">
              <option :value="0">{{ t("set.speedAuto") }}</option>
              <option :value="1">1.0x</option>
              <option :value="1.25">1.25x</option>
              <option :value="1.5">1.5x</option>
              <option :value="1.8">1.8x</option>
              <option :value="2">2.0x</option>
              <option :value="2.5">{{ t("set.speed25") }}</option>
              <option :value="3">{{ t("set.speed30") }}</option>
            </select>
          </label>
          <p class="setting-tip">{{ t("set.speedTip") }}</p>
        </div>

        <div class="setting-item card">
          <label>
            {{ t("set.threshold", Math.round(cfg.end_threshold * 100)) }}
            <input
              type="range"
              min="0.85"
              max="1"
              step="0.01"
              v-model.number="cfg.end_threshold"
            />
          </label>
          <p class="setting-tip">{{ t("set.thresholdTip") }}</p>
        </div>

        <div class="setting-item card">
          <label>
            {{ t("set.limit") }}
            <input type="text" v-model.number="cfg.limit_minutes" />
          </label>
          <p class="setting-tip">{{ t("set.limitTip") }}</p>
        </div>

        <div class="setting-item card">
          <label>
            {{ t("set.lang") }}
            <select :value="lang" @change="onLangChange">
              <option v-for="l in langs" :key="l.value" :value="l.value">
                {{ l.label }}
              </option>
            </select>
          </label>
          <p class="setting-tip">{{ t("set.langTip") }}</p>
        </div>

        <div class="setting-item card">
          <div class="toggle-row">
            <div>
              <b>{{ t("set.tray") }}</b>
              <p class="setting-tip">{{ t("set.trayTip") }}</p>
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
              <b>{{ t("set.dark") }}</b>
              <p class="setting-tip">{{ t("set.darkTip") }}</p>
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
    </section>
    </main>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, reactive, watch } from "vue";
import {
  api,
  onCourseEvent,
  onQueueState,
  parseErr,
  type AppConfig,
  type CourseItem,
  type QueueState,
} from "../api";
import { uiAlert, uiConfirm } from "../dialog";
import { t, lang, setLang, langs, type Lang } from "../i18n";

const emit = defineEmits<{ logout: [] }>();

const tabs = [
  { key: "courses", labelKey: "nav.courses", icon: "📚" },
  { key: "run", labelKey: "nav.run", icon: "⚡" },
  { key: "settings", labelKey: "nav.settings", icon: "⚙️" },
] as const;

const tab = ref<"courses" | "run" | "settings">("courses");

// ---------- 课程列表（合并为一个可拖拽排序的列表） ----------
interface CourseRow {
  name: string;
  id: string;
  type: "zhidao" | "hike";
}

const courseList = ref<CourseRow[]>([]);
const loadingCourses = ref(false);
const coursesError = ref("");
const pickedIds = ref(new Set<string>());

const ORDER_KEY = "fzhs-course-order";

function typeLabel(type: "zhidao" | "hike"): string {
  return type === "zhidao" ? t("type.share") : t("type.school");
}

/** 按保存的顺序排列课程（新课程追加在末尾） */
function applyOrder(rows: CourseRow[]): CourseRow[] {
  let saved: string[] = [];
  try {
    saved = JSON.parse(localStorage.getItem(ORDER_KEY) || "[]");
  } catch {
    /* ignore */
  }
  const idx = new Map(saved.map((id, i) => [id, i]));
  return [...rows].sort(
    (a, b) =>
      (idx.get(a.id) ?? Number.MAX_SAFE_INTEGER) -
      (idx.get(b.id) ?? Number.MAX_SAFE_INTEGER)
  );
}

function persistOrder() {
  try {
    localStorage.setItem(
      ORDER_KEY,
      JSON.stringify(courseList.value.map((c) => c.id))
    );
  } catch {
    /* 存储不可用（配额/策略）时静默，不影响拖拽操作 */
  }
}

async function loadCourses() {
  loadingCourses.value = true;
  coursesError.value = "";
  try {
    const r = await api.getCourses();
    const rows: CourseRow[] = [
      ...r.zhidao.map((c) => ({ ...c, type: "zhidao" as const })),
      ...r.hike.map((c) => ({ ...c, type: "hike" as const })),
    ];
    courseList.value = applyOrder(rows);
  } catch (e) {
    coursesError.value = t("err.loadCourses", parseErr(e).message);
  } finally {
    loadingCourses.value = false;
  }
}

const allPicked = computed(
  () =>
    courseList.value.length > 0 &&
    courseList.value.every((c) => pickedIds.value.has(c.id))
);

// ---------- 拖拽排序（手柄 + 指针事件；HTML5 DnD 在 WebView2 里拖不动） ----------
// 拿起后卡片变"幽灵"跟随鼠标（放大动效），原位置显示占位框，松手缩回吸附到落点。
const courseListBox = ref<HTMLElement | null>(null);
const dragId = ref<string | null>(null);
const dragIndex = ref(0);
const dragH = ref(48);
const dragDropping = ref(false);
const ghostStyle = ref<Record<string, string>>({});
let dragStartY = 0;
let dragOrigTop = 0;
let dragListTop = 0;
let dragSlotH = 1;
let dragMoved = false;
let dropTimer: number | null = null;

const dragCourse = computed(
  () => courseList.value.find((c) => c.id === dragId.value) || null
);

/** 重新采样列表几何（拖拽中滚动时调用，避免落点错位）。
 *  只取真实课程行，排除末尾的全选行。 */
function sampleDragGeometry(from: number): { top: number; height: number; selfLeft: number; selfWidth: number; selfTop: number } | null {
  const box = courseListBox.value;
  if (!box) return null;
  const rows = Array.from(box.querySelectorAll(".course-list > .course-item")) as HTMLElement[];
  if (rows.length < 2) return null;
  const r0 = rows[0].getBoundingClientRect();
  const r1 = rows[1].getBoundingClientRect();
  const row = rows[Math.min(from, rows.length - 1)].getBoundingClientRect();
  return {
    top: r0.top,
    height: r1.top - r0.top,
    selfLeft: row.left,
    selfWidth: row.width,
    selfTop: row.top,
  };
}

function onHandleDown(id: string, e: MouseEvent) {
  if (dropTimer) {
    clearTimeout(dropTimer);
    dropTimer = null;
  }
  const list = courseList.value;
  const from = list.findIndex((c) => c.id === id);
  if (from < 0 || list.length < 2) return;
  const g = sampleDragGeometry(from);
  if (!g) return;
  dragListTop = g.top;
  dragSlotH = g.height;
  dragH.value = Math.round(g.height);
  dragStartY = e.clientY;
  dragOrigTop = g.selfTop;
  dragMoved = false;
  dragDropping.value = false;
  dragIndex.value = from;
  ghostStyle.value = {
    left: g.selfLeft + "px",
    top: g.selfTop + "px",
    width: g.selfWidth + "px",
  };
  dragId.value = id;
  window.addEventListener("mousemove", onDragMove);
  window.addEventListener("mouseup", onDragFinish);
  window.addEventListener("blur", onDragFinish);
  window.addEventListener("wheel", onDragScroll, { passive: true });
}

/** 拖拽中滚动：幽灵的绝对坐标要跟着内容一起移动，否则吸附位置错乱 */
function onDragScroll() {
  if (!dragId.value || dragDropping.value) return;
  const g = sampleDragGeometry(dragIndex.value);
  if (!g) return;
  const dy = g.top - dragListTop;
  dragListTop = g.top;
  dragOrigTop += dy;
  ghostStyle.value = { ...ghostStyle.value, top: parseFloat(ghostStyle.value.top || "0") + dy + "px" };
}

function onDragMove(e: MouseEvent) {
  if (!dragId.value || dragDropping.value) return;
  if (!(e.buttons & 1)) {
    onDragFinish();
    return;
  }
  // 幽灵卡片跟手（垂直方向）
  ghostStyle.value = {
    ...ghostStyle.value,
    top: dragOrigTop + (e.clientY - dragStartY) + "px",
  };
  // 光标进入哪个槽位就换到哪个位置
  const len = courseList.value.length;
  const idx = Math.min(
    len - 1,
    Math.max(0, Math.floor((e.clientY - dragListTop) / dragSlotH))
  );
  if (idx === dragIndex.value) return;
  const from = courseList.value.findIndex((c) => c.id === dragId.value);
  if (from < 0) return;
  const [moved] = courseList.value.splice(from, 1);
  courseList.value.splice(idx, 0, moved);
  dragIndex.value = idx;
  dragMoved = true;
}

function onDragFinish() {
  if (!dragId.value) return;
  window.removeEventListener("mousemove", onDragMove);
  window.removeEventListener("mouseup", onDragFinish);
  window.removeEventListener("blur", onDragFinish);
  window.removeEventListener("wheel", onDragScroll);
  if (dragMoved) persistOrder();
  // 缩回原大小并吸附到落点槽位，然后移除幽灵、显示真实卡片
  dragDropping.value = true;
  ghostStyle.value = {
    ...ghostStyle.value,
    top: dragListTop + dragIndex.value * dragSlotH + "px",
  };
  dropTimer = window.setTimeout(() => {
    dragId.value = null;
    dragDropping.value = false;
    dropTimer = null;
  }, 170);
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
    for (const c of courseList.value) s.add(c.id);
  }
  pickedIds.value = s;
}

/** 停止全部任务并把刷课界面还原为初始状态（开始刷课 / 清空全部共用） */
async function resetRunView() {
  for (const id of runs.keys()) clearedIds.value.add(id);
  for (const q of queueState.pending) clearedIds.value.add(q.id);
  await api.stopAll();
  runs.clear();
  skippedIds.value.clear();
  captchaNotice.value = false;
  // queue-state 事件会同步清空 pending/current/paused
}

async function startPicked() {
  const picked = courseList.value.filter((c) => pickedIds.value.has(c.id));
  if (picked.length === 0) return;
  try {
    // 每次开始刷课：清空上一轮的列表，从新选择重新开始
    await resetRunView();
    const failed = await api.startCourses(picked as CourseItem[]);
    for (const c of picked) {
      doneIds.value.delete(c.id); // 重新开刷，摘掉"已完成"直到再次刷完
      // 【防旧尾事件污染】延时解除屏蔽：旧任务的 done("stopped") 尾事件
      // 往往晚于 startCourses 的返回，过早删除会让旧尾事件把新卡片标成"已停止"。
      window.setTimeout(() => clearedIds.value.delete(c.id), 3000);
    }
    persistDone();
    tab.value = "run";
    pickedIds.value = new Set();
    if (failed.length > 0) {
      await uiAlert(t("err.startDup", failed.length));
    }
  } catch (e) {
    await uiAlert(t("err.start", parseErr(e).message));
  }
}

// ---------- 队列状态 ----------
const queueState = reactive<{
  current: { id: string; name: string } | null;
  pending: { id: string; name: string }[];
  paused: boolean;
}>({ current: null, pending: [], paused: false });

const captchaNotice = ref(false);

function applyQueueState(s: QueueState) {
  queueState.current = s.current;
  queueState.pending = s.pending;
  queueState.paused = s.paused;
  if (s.notice === "captcha_cleared") captchaNotice.value = true;
  // 【防僵尸卡片】清空/重新开始时，旧的 queue-state 快照可能仍带着旧 current，
  // 若此时重建 run 卡片，随后的 done 尾事件又被 clearedIds 屏蔽 → 卡片永久卡"准备中"。
  // 被清掉的 id 一律不再由队列快照复活。
  if (s.current && !clearedIds.value.has(s.current.id)) {
    ensureRun(s.current.id, s.current.name);
  }
}

function isCurrent(id: string) {
  return queueState.current?.id === id;
}

function isQueued(id: string) {
  return queueState.pending.some((c) => c.id === id);
}

/** 已刷完（课程列表页显示"已完成"标记，清空进度/重启后仍保留） */
function isDone(id: string) {
  return doneIds.value.has(id);
}

async function togglePause() {
  await api.setQueuePaused(!queueState.paused);
}

/** 清空全部：还在刷（进行中或排队）时先确认；已经停了就直接清 */
async function clearAll() {
  if (runningCount.value > 0 && !(await uiConfirm(t("confirm.clearAll"), { danger: true }))) {
    return;
  }
  await resetRunView();
}

function removeQueued(id: string) {
  api.stopCourse(id); // 排队中的课程：后端直接移出队列
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
  /** 终态标签的 i18n key：tag.done / tag.stopped / tag.skipped / tag.captcha / tag.limit / tag.error */
  doneTag: string;
  /** 账号被智慧树临时限制（-12），卡片上展示恢复引导 */
  captcha: boolean;
}

const runs = reactive(new Map<string, RunState>());
const skippedIds = ref(new Set<string>());
/** 「清空全部」时记录被清掉的课程：忽略旧任务随后发来的尾巴事件，避免卡片复活 */
const clearedIds = ref(new Set<string>());
/** 刷完的课程（持久化）：清空刷课进度/重启后，「我的课程」仍显示"已完成" */
const DONE_KEY = "fzhs-done-courses";
const doneIds = ref(new Set<string>(loadDoneIds()));

function loadDoneIds(): string[] {
  try {
    return JSON.parse(localStorage.getItem(DONE_KEY) || "[]");
  } catch {
    return [];
  }
}

function persistDone() {
  try {
    localStorage.setItem(DONE_KEY, JSON.stringify([...doneIds.value]));
  } catch {
    /* 存储不可用时静默 */
  }
}

const runningCount = computed(
  () => [...runs.values()].filter((r) => !r.done).length + queueState.pending.length
);

type RunRow =
  | { rowType: "active"; run: RunState }
  | { rowType: "queued"; course: { id: string; name: string }; pos: number }
  | { rowType: "done"; run: RunState };

/** 排序：正在刷的置顶 -> 排队中 -> 已刷完的放最后 */
const runRows = computed<RunRow[]>(() => {
  const rows: RunRow[] = [];
  for (const r of runs.values()) {
    if (!r.done) rows.push({ rowType: "active", run: r });
  }
  queueState.pending.forEach((c, i) =>
    rows.push({ rowType: "queued", course: c, pos: i + 1 })
  );
  for (const r of runs.values()) {
    if (r.done) rows.push({ rowType: "done", run: r });
  }
  return rows;
});

function rowKey(row: RunRow): string {
  return row.rowType === "queued" ? "q:" + row.course.id : row.run.id;
}

function sectionTitle(row: RunRow, i: number): string | null {
  const prev = i > 0 ? runRows.value[i - 1] : null;
  if (row.rowType === "active") return i === 0 ? "run.section.active" : null;
  if (row.rowType === "queued")
    return prev?.rowType === "queued" ? null : "run.section.queue";
  return prev?.rowType === "done" ? null : "run.section.done";
}

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
  const time = new Date().toLocaleTimeString([], { hour12: false });
  r.logs.push({ level, msg: `[${time}] ${msg}` });
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

function skipReasonText(reason?: string): string {
  if (!reason) return "";
  if (["watched", "quiz", "resource"].includes(reason)) {
    return t("reason." + reason);
  }
  return reason;
}

let unlisten: (() => void) | null = null;
let unlistenQueue: (() => void) | null = null;
let homeUnmounted = false;

onMounted(async () => {
  const unEvent = await onCourseEvent(({ courseId, courseName, event }) => {
    // 「清空全部」后忽略旧任务的尾巴事件（它们到达时 runs 已被清空）
    if (clearedIds.value.has(courseId)) return;
    const r = ensureRun(courseId, courseName);
    switch (event.kind) {
      case "log": {
        const code = event.code || "";
        if (code === "task_start") pushLog(r, "info", t("log.taskStart"));
        else if (code === "loading_course")
          pushLog(r, "info", t("log.loading", event.arg || ""));
        else if (code === "start_course")
          pushLog(r, "info", t("log.startCourse", event.arg || ""));
        else pushLog(r, event.level || "info", event.msg || "");
        break;
      }
      case "chapter":
        r.chapter = event.name || "";
        pushLog(r, "info", t("log.chapter", r.chapter));
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
        pushLog(r, "info", t("log.videoStart", event.name || ""));
        break;
      case "video_progress":
        // 后端 VideoProgress 事件只有 video_id 没有 name，必须按 video_id 匹配；
        // 应用重启后进度事件先到（video_start 已丢失），用事件数据重建，
        // 避免当前视频整段停在"准备中"
        if (r.video && r.video.video_id === event.video_id) {
          r.video.played = event.played || 0;
          r.video.total = event.total || 0;
          r.video.paused = !!event.paused;
          r.video.quiz = !!event.quiz;
        } else if (!r.video && event.video_id) {
          r.video = {
            video_id: event.video_id,
            name: event.name || event.video_id,
            played: event.played || 0,
            total: event.total || 0,
            paused: !!event.paused,
            quiz: !!event.quiz,
          };
        }
        break;
      case "video_done":
        pushLog(r, "info", t("log.videoDone", event.name || ""));
        r.video = null;
        break;
      case "quiz_answered":
        pushLog(r, "ok", t("log.quizAnswered", event.question || ""));
        break;
      case "skipped":
        pushLog(
          r,
          "info",
          t("log.skipped", event.name || "", skipReasonText(event.reason))
        );
        break;
      case "limit_reached":
        pushLog(r, "warn", t("log.limit"));
        r.done = true;
        r.doneTag = "tag.limit";
        break;
      case "captcha_required":
        r.done = true;
        r.captcha = true;
        r.doneTag = "tag.captcha";
        pushLog(r, "warn", t("log.captchaHit"));
        break;
      case "done": {
        const code = event.code || "";
        let line: string;
        if (code === "course_finished") line = t("done.courseFinished");
        else if (code === "finished") line = t("done.finished");
        else if (code === "stopped") line = t("done.stopped");
        else if (code === "captcha") line = t("done.captcha");
        else line = event.msg || "";
        pushLog(r, code === "error" ? "error" : "ok", line);
        r.done = true;
        if (code === "captcha" || (!code && (event.msg || "").includes("限制"))) {
          // -12 在任务开头就被拦下时不会发 captcha_required，靠这里兜底展示引导
          r.captcha = true;
          r.doneTag = "tag.captcha";
        } else if (code === "stopped") {
          r.doneTag = skippedIds.value.has(courseId) ? "tag.skipped" : "tag.stopped";
        } else if (code === "error") {
          r.doneTag = "tag.error";
        } else if (!r.doneTag) {
          // finished：limit_reached 等已先设好终态标签时保留（那种不算刷完）
          r.doneTag = "tag.done";
          doneIds.value.add(courseId);
          persistDone();
        }
        skippedIds.value.delete(courseId);
        break;
      }
      case "failed": {
        pushLog(r, "error", event.msg || "");
        // 关键失败（账号被限制）时任务实际已停止，显示终态避免进度条凝固
        const msg = event.msg || "";
        if (msg.includes("限制") || msg.includes("验证码")) {
          r.done = true;
          r.captcha = true;
          r.doneTag = "tag.captcha";
        }
        break;
      }
    }
  });
  if (homeUnmounted) unEvent();
  else unlisten = unEvent;

  const unQueue = await onQueueState(applyQueueState);
  if (homeUnmounted) unQueue();
  else unlistenQueue = unQueue;

  // 恢复队列与运行状态（页面刷新时）
  try {
    applyQueueState(await api.getQueueState());
  } catch {
    /* ignore */
  }
  await loadCourses();
  await loadCfg();
});

onBeforeUnmount(() => {
  homeUnmounted = true;
  if (unlisten) unlisten();
  if (unlistenQueue) unlistenQueue();
  onDragFinish();
  if (dropTimer) {
    clearTimeout(dropTimer);
    dropTimer = null;
  }
  // 未执行的设置保存不能丢（用户改完立刻退出登录/关窗）
  if (saveTimer) {
    clearTimeout(saveTimer);
    saveTimer = null;
    if (cfgLoaded) void doSaveCfg();
  }
});

async function skipCurrent(id: string) {
  skippedIds.value.add(id);
  try {
    await api.stopCourse(id); // 正在刷的课程：停止后队列自动开始下一门
  } catch {
    skippedIds.value.delete(id);
  }
}

/** 完成人工验证后，从卡片上直接重新开始这门课 */
async function restartRun(r: RunState) {
  r.captcha = false;
  captchaNotice.value = false;
  // 延时解除屏蔽，避免旧 captcha 尾事件覆盖重刷状态
  window.setTimeout(() => clearedIds.value.delete(r.id), 3000);
  doneIds.value.delete(r.id);
  persistDone();
  try {
    const failed = await api.startCourses([{ id: r.id, name: r.name }]);
    if (failed.length === 0) {
      ensureRun(r.id, r.name);
    } else {
      await uiAlert(t("err.startDup", 1));
    }
  } catch (e) {
    await uiAlert(t("err.start", parseErr(e).message));
  }
}

async function doLogout() {
  if (runningCount.value > 0 && !(await uiConfirm(t("confirm.logout")))) return;
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

// 黑暗模式（UI 偏好存 localStorage，不走后端配置）
const darkMode = ref(localStorage.getItem("fzhs-dark") === "1");

function applyDark() {
  document.documentElement.classList.toggle("dark", darkMode.value);
  localStorage.setItem("fzhs-dark", darkMode.value ? "1" : "0");
}

function onLangChange(e: Event) {
  setLang((e.target as HTMLSelectElement).value as Lang);
}

async function loadCfg() {
  cfg.value = await api.getConfig();
  cfgLoaded = true;
}

// 设置即改即存（无保存按钮、无提示）：深度监听 + 300ms 防抖
let cfgLoaded = false;
let saveTimer: number | null = null;

/** 构造干净配置（非法输入不应让后端反序列化失败）：时限必须是非负整数。
 *  注意不写回 cfg ref，否则会再次触发 watch 造成循环。 */
function cleanCfg(): AppConfig {
  const n = Number(cfg.value.limit_minutes);
  const s = Number(cfg.value.speed);
  return {
    ...cfg.value,
    limit_minutes: Number.isFinite(n) && n >= 0 ? Math.floor(n) : 0,
    speed: Number.isFinite(s) && s >= 0 ? s : 0,
  };
}

async function doSaveCfg() {
  try {
    await api.saveConfig(cleanCfg());
  } catch {
    /* 保存失败静默，下次修改再试 */
  }
}

watch(
  cfg,
  () => {
    if (!cfgLoaded) return;
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = window.setTimeout(async () => {
      saveTimer = null;
      await doSaveCfg();
    }, 300);
  },
  { deep: true }
);
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
  font-size: 16px;
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

/* 图标统一盒子：三行图标同宽对齐，字号一致 */
.nav-icon {
  width: 22px;
  flex-shrink: 0;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-size: 16px;
  line-height: 1;
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

.start-btn {
  margin-left: auto;
}

/* 全选行：列表末尾的最后一个框；空序号占位使复选框与条目对齐 */
.pick-all-row label {
  color: var(--text-sub);
}

.drag-tip {
  color: var(--text-sub);
  font-size: 12.5px;
  margin: -6px 0 0;
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
  gap: 10px;
  cursor: pointer;
  font-size: 15px;
  min-width: 0;
  flex: 1;
}

.course-item.running {
  opacity: 0.75;
}

.course-item.is-queued {
  opacity: 0.85;
}

.course-item.dragging {
  opacity: 0.5;
}

/* 序号（跟随拖拽排序实时变化） */
.course-no {
  width: 24px;
  flex-shrink: 0;
  text-align: right;
  color: var(--text-sub);
  font-size: 12.5px;
  font-variant-numeric: tabular-nums;
}

.course-name {
  max-width: 520px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* 课程性质：彩色 dot + 小字 */
.dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}

.dot-share {
  background: var(--green);
}

.dot-school {
  background: #4a90d9;
}

.type-label {
  font-size: 11.5px;
  color: var(--text-sub);
  background: var(--tab-bg);
  padding: 1px 8px;
  border-radius: 8px;
  flex-shrink: 0;
}

/* 拖拽手柄：只有拖住它才能调整顺序 */
.course-right {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}

/* 拖拽占位框：原位置留出的空槽 */
.drag-placeholder {
  border: 1.5px dashed var(--scroll-thumb);
  background: var(--tab-bg);
  border-radius: 10px;
  margin-bottom: 8px;
}

/* 拖拽幽灵卡片：跟随鼠标，拿起放大、松手缩回（top 无过渡保证跟手） */
.drag-ghost {
  position: fixed;
  z-index: 100;
  margin: 0;
  pointer-events: none;
  user-select: none;
  box-shadow: 0 10px 28px var(--card-shadow);
  transform: scale(1.03);
  transition: transform 0.15s ease, box-shadow 0.15s ease;
}

.drag-ghost.dropping {
  transform: scale(1);
  box-shadow: 0 2px 8px var(--card-shadow);
  transition: transform 0.15s ease, top 0.16s ease, box-shadow 0.15s ease;
}

.drag-handle {
  cursor: grab;
  color: var(--text-sub);
  font-size: 15px;
  line-height: 1;
  padding: 3px 7px;
  border-radius: 6px;
  user-select: none;
  flex-shrink: 0;
}

.drag-handle:hover {
  background: var(--tab-bg);
  color: var(--text);
}

.drag-handle:active {
  cursor: grabbing;
}

.course-tag {
  color: var(--green-dark);
  background: var(--green-light);
  font-size: 12px;
  padding: 3px 10px;
  border-radius: 10px;
  flex-shrink: 0;
}

.queued-tag {
  color: #c07f00;
  background: rgba(240, 173, 40, 0.16);
}

html.dark .queued-tag {
  color: #ffc861;
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

/* ---------- 运行页 ---------- */
.run-toolbar {
  display: flex;
  align-items: center;
  gap: 10px;
}

.clear-all {
  color: var(--danger);
}

.paused-tip {
  color: #c07f00;
  font-size: 13px;
}

html.dark .paused-tip {
  color: #ffc861;
}

.queue-notice {
  padding: 10px 14px;
  border-radius: 10px;
  background: rgba(240, 173, 40, 0.12);
  border: 1px solid rgba(240, 173, 40, 0.5);
  color: #c07f00;
  font-size: 13px;
  line-height: 1.6;
}

html.dark .queue-notice {
  color: #ffc861;
}

.section-title {
  font-size: 15px;
  margin: 4px 0 -2px;
  color: var(--text);
}

.queue-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 11px 18px;
}

.queue-no {
  width: 22px;
  height: 22px;
  border-radius: 50%;
  background: var(--tab-bg);
  color: var(--text-sub);
  font-size: 12.5px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.queue-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 14.5px;
}

.btn-ghost.sm {
  padding: 5px 12px;
  font-size: 13px;
}

.run-card {
  padding: 16px 20px;
}

.run-card.is-current {
  border-left: 3px solid var(--green);
}

.run-card.is-done {
  opacity: 0.78;
}

.run-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 10px;
  gap: 10px;
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
  gap: 10px;
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

/* ---------- 设置：最多一行两块，窄窗口单列 ---------- */
.settings-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  grid-auto-rows: 1fr;
  gap: 14px;
}

@media (max-width: 720px) {
  .settings-grid {
    grid-template-columns: 1fr;
  }
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
</style>
