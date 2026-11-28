// 轻量多语言：简体中文 / 繁體中文 / English。
// 无依赖：t() 在渲染期读取 lang（ref），切换语言自动刷新界面。
// 约定：文案里的 {0} {1} 为占位符，按 t() 的参数顺序替换。

import { ref } from "vue";

export type Lang = "zh-CN" | "zh-TW" | "en";

const STORAGE_KEY = "fzhs-lang";

export const langs: { value: Lang; label: string }[] = [
  { value: "zh-CN", label: "简体中文" },
  { value: "zh-TW", label: "繁體中文" },
  { value: "en", label: "English" },
];

export const lang = ref<Lang>(
  (localStorage.getItem(STORAGE_KEY) as Lang) || "zh-CN"
);

export function setLang(l: Lang) {
  lang.value = l;
  localStorage.setItem(STORAGE_KEY, l);
}

const zhCN: Record<string, string> = {
  // 导航
  "nav.courses": "我的课程",
  "nav.run": "刷课进度",
  "nav.settings": "设置",
  "side.logout": "↩ 退出登录",

  // 课程页
  "btn.refresh": "刷新课程列表",
  "btn.refreshing": "加载中…",
  "pickAll": "全选",
  "btn.start": "开始刷课（{0} 门）",
  "type.share": "共享课",
  "type.school": "校内课",
  "tag.running": "刷课中",
  "tag.queued": "排队中",
  "dragTip": "拖住课程名后的手柄图标可调整刷课顺序",
  "empty.courses.title": "没有查到课程 😢",
  "empty.courses.sub":
    "可能原因：账号下没有课程 / 登录已失效 / 网络异常，可点上方按钮刷新。",
  "err.loadCourses": "课程加载失败：{0}",
  "err.start": "启动失败：{0}",
  "err.startDup": "{0} 门课程已在队列里，未重复添加",

  // 刷课页
  "run.empty.title": "还没有任务在跑",
  "run.empty.sub":
    "去「我的课程」勾选课程，点「开始刷课」即可。关掉窗口也会在后台继续刷。",
  "run.section.active": "正在刷课",
  "run.section.queue": "排队中（{0}）",
  "run.section.done": "已完成 / 已停止",
  "btn.pause": "⏸ 暂停",
  "btn.resume": "▶ 继续",
  "btn.clearAll": "🗑 清空全部",
  "btn.skip": "跳过此课程",
  "btn.remove": "移除",
  "run.pausedTip": "已暂停：当前视频进度冻结，点「继续」恢复",
  "notice.captchaCleared":
    "账号被智慧树临时限制，剩余排队课程已清空。完成人工验证后可重新开始。",
  "state.paused": "已暂停",
  "state.simPause": "模拟暂停",
  "state.quiz": "答题中…",
  "run.preparing": "准备中…",
  "tag.done": "已完成",
  "tag.stopped": "已停止",
  "tag.skipped": "已跳过",
  "tag.captcha": "待验证",
  "tag.limit": "已达时限",
  "tag.error": "已出错",
  "captcha.title": "⚠️ 账号被智慧树临时限制，刷课已暂停（不是软件坏了）",
  "captcha.intro":
    "被限制后有一个 30~60 分钟的账号冷却期——冷却期内无论怎么重试都会再次失败，且每失败一次限制就加重一分。请严格按下面顺序操作，不要跳步、不要提前重试：",
  "captcha.s1": "① 先等 30~60 分钟（期间千万别点重新开始，越早点越严重）",
  "captcha.s2": "② 用浏览器登录智慧树网页版，手动播放任意视频 1~2 分钟",
  "captcha.s3": "③ 回到这里，点左侧底部「退出登录」→ 重新扫码登录",
  "captcha.s4": "④ 先只挂一门课，等它跑完再挂下一门",
  "captcha.retry": "我已完成验证，重新开始刷课",
  "confirm.logout": "还有任务在刷课，退出登录将全部停止，确定？",
  "confirm.clearAll": "此操作不可撤销，确认删除？",
  "confirm.cooldown":
    "账号在 {0} 分钟前刚被限制，还在风控冷却期内（约 30~60 分钟）。\n\n现在重试大概率几分钟内又会失败，并且每次失败都会加重限制、延长冷却。\n\n建议：先等够 30~60 分钟 → 用浏览器登录智慧树网页版手动看 1~2 分钟视频 → 再重新扫码登录 → 只挂一门课。确定仍要立即重试？",
  "dlg.ok": "确定",
  "dlg.cancel": "取消",

  // 日志
  "log.taskStart": "任务开始",
  "log.loading": "正在加载课程 {0} …",
  "log.startCourse": "开始刷课：{0}",
  "log.chapter": "章节：{0}",
  "log.videoStart": "开始观看：{0}",
  "log.videoDone": "看完：{0}",
  "log.quizAnswered": "已自动答题：{0}",
  "log.skipped": "跳过 {0}：{1}",
  "reason.watched": "已看完",
  "reason.quiz": "可能是测验，不支持自动完成",
  "reason.resource": "非视频资源，已访问",
  "log.limit": "达到单课程时限，本课程停止",
  "log.captchaHit": "账号被智慧树临时限制，任务已暂停（见上方提示）",
  "log.manualStop": "已手动停止",
  "done.courseFinished": "课程完成",
  "done.finished": "任务结束",
  "done.stopped": "任务已停止",
  "done.captcha": "任务已暂停：账号被智慧树临时限制（见上方提示）",

  // 设置页
  "set.speed": "播放倍速",
  "set.speedAuto": "平台最高（推荐，共享课 1.5x / 校内课 1.25x）",
  "set.speed25": "2.5x（快，风控风险略高）",
  "set.speed30": "3.0x（极速，风控风险高）",
  "set.speedTip":
    "默认就是最稳的选择；倍速越高刷得越快，被智慧树盯上的概率也越大。",
  "set.threshold": "完成阈值：{0}%",
  "set.thresholdTip": "播放进度达到该百分比即视为学完，默认 100% 刷满整个视频。",
  "set.limit": "单门课程时限（分钟，0 = 不限）",
  "set.limitTip": "建议参考课程学习习惯中的建议学习时长。",
  "set.tray": "最小化到托盘",
  "set.trayTip": "开启时关闭窗口缩到托盘，后台继续刷课；关闭时点 × 直接退出程序。",
  "set.dark": "黑暗模式",
  "set.darkTip": "深色界面，夜间刷课更护眼，立即生效。",
  "set.lang": "界面语言",
  "set.langTip": "切换后立即生效，无需保存。",

  // 登录页
  "login.slogan": "把网课丢给它，你去吃点好的",
  "login.tabQr": "扫码登录（推荐）",
  "login.tabBrowser": "浏览器登录",
  "login.qrAlt": "登录二维码",
  "login.qrFetching": "正在获取二维码…",
  "login.qrScanned": "✓ 已扫码<br />确认后正在登录…",
  "login.qrHint": "📱 打开<b>知到 APP</b>，扫一扫上面的二维码",
  "login.qrRefresh": "刷新二维码",
  "login.browserHint":
    "点击下方按钮后，本窗口会变成智慧树登录页面。<br />在页面里输入账号密码（需要滑块/短信验证也直接完成），<br /><b>登录成功后本程序会自动返回并进入主界面</b>。",
  "login.openBrowser": "打开登录页面",
  "login.opening": "正在打开…",
  "login.browserTip": "推荐使用「扫码登录」，无需输入账号密码更安全。",
  "login.cancelled": "已在手机上取消登录，请点下方按钮刷新二维码重试",
  "login.expired": "二维码已过期，正在自动刷新…",
  "login.retry": "（点此重试）",
  "login.netRetry": "（请检查网络后点下方按钮重试）",
  "login.footer":
    "仅供学习交流 · 请合理安排时间 · 本工具不上传任何数据到第三方",

  // 其他
  "boot": "正在启动…",
};

const zhTW: Record<string, string> = {
  "nav.courses": "我的課程",
  "nav.run": "刷課進度",
  "nav.settings": "設定",
  "side.logout": "↩ 登出",

  "btn.refresh": "重新整理課程列表",
  "btn.refreshing": "載入中…",
  "pickAll": "全選",
  "btn.start": "開始刷課（{0} 門）",
  "type.share": "共享課",
  "type.school": "校內課",
  "tag.running": "刷課中",
  "tag.queued": "排隊中",
  "dragTip": "拖住課程名稱後的手柄圖示可調整刷課順序",
  "empty.courses.title": "沒有查到課程 😢",
  "empty.courses.sub":
    "可能原因：帳號下沒有課程 / 登入已失效 / 網路異常，可點上方按鈕重新整理。",
  "err.loadCourses": "課程載入失敗：{0}",
  "err.start": "啟動失敗：{0}",
  "err.startDup": "{0} 門課程已在佇列裡，未重複加入",

  "run.empty.title": "還沒有任務在跑",
  "run.empty.sub":
    "到「我的課程」勾選課程，點「開始刷課」即可。關掉視窗也會在背景繼續刷。",
  "run.section.active": "正在刷課",
  "run.section.queue": "排隊中（{0}）",
  "run.section.done": "已完成 / 已停止",
  "btn.pause": "⏸ 暫停",
  "btn.resume": "▶ 繼續",
  "btn.clearAll": "🗑 清空全部",
  "btn.skip": "跳過此課程",
  "btn.remove": "移除",
  "run.pausedTip": "已暫停：目前影片進度凍結，點「繼續」恢復",
  "notice.captchaCleared":
    "帳號被智慧樹臨時限制，剩餘排隊課程已清空。完成人工驗證後可重新開始。",
  "state.paused": "已暫停",
  "state.simPause": "模擬暫停",
  "state.quiz": "答題中…",
  "run.preparing": "準備中…",
  "tag.done": "已完成",
  "tag.stopped": "已停止",
  "tag.skipped": "已跳過",
  "tag.captcha": "待驗證",
  "tag.limit": "已達時限",
  "tag.error": "已出錯",
  "captcha.title": "⚠️ 帳號被智慧樹臨時限制，刷課已暫停（不是軟體壞了）",
  "captcha.intro":
    "被限制後有一個 30~60 分鐘的帳號冷卻期——冷卻期內無論怎麼重試都會再次失敗，且每失敗一次限制就加重一分。請嚴格按下面順序操作，不要跳步、不要提前重試：",
  "captcha.s1": "① 先等 30~60 分鐘（期間千萬別點重新開始，越早點越嚴重）",
  "captcha.s2": "② 用瀏覽器登入智慧樹網頁版，手動播放任意影片 1~2 分鐘",
  "captcha.s3": "③ 回到這裡，點左側底部「登出」→ 重新掃碼登入",
  "captcha.s4": "④ 先只掛一門課，等它跑完再掛下一門",
  "captcha.retry": "我已完成驗證，重新開始刷課",
  "confirm.logout": "還有任務在刷課，登出將全部停止，確定？",
  "confirm.clearAll": "此操作不可撤銷，確認刪除？",
  "confirm.cooldown":
    "帳號在 {0} 分鐘前剛被限制，還在風控冷卻期內（約 30~60 分鐘）。\n\n現在重試大概率幾分鐘內又會失敗，並且每次失敗都會加重限制、延長冷卻。\n\n建議：先等夠 30~60 分鐘 → 用瀏覽器登入智慧樹網頁版手動看 1~2 分鐘影片 → 再重新掃碼登入 → 只掛一門課。確定仍要立即重試？",
  "dlg.ok": "確定",
  "dlg.cancel": "取消",

  "log.taskStart": "任務開始",
  "log.loading": "正在載入課程 {0} …",
  "log.startCourse": "開始刷課：{0}",
  "log.chapter": "章節：{0}",
  "log.videoStart": "開始觀看：{0}",
  "log.videoDone": "看完：{0}",
  "log.quizAnswered": "已自動答題：{0}",
  "log.skipped": "跳過 {0}：{1}",
  "reason.watched": "已看完",
  "reason.quiz": "可能是測驗，不支援自動完成",
  "reason.resource": "非影片資源，已訪問",
  "log.limit": "達到單課程時限，本課程停止",
  "log.captchaHit": "帳號被智慧樹臨時限制，任務已暫停（見上方提示）",
  "log.manualStop": "已手動停止",
  "done.courseFinished": "課程完成",
  "done.finished": "任務結束",
  "done.stopped": "任務已停止",
  "done.captcha": "任務已暫停：帳號被智慧樹臨時限制（見上方提示）",

  "set.speed": "播放倍速",
  "set.speedAuto": "平台最高（推薦，共享課 1.5x / 校內課 1.25x）",
  "set.speed25": "2.5x（快，風控風險略高）",
  "set.speed30": "3.0x（極速，風控風險高）",
  "set.speedTip":
    "預設就是最穩的選擇；倍速越高刷得越快，被智慧樹盯上的機率也越大。",
  "set.threshold": "完成門檻：{0}%",
  "set.thresholdTip": "播放進度達到該百分比即視為學完，預設 100% 刷滿整部影片。",
  "set.limit": "單門課程時限（分鐘，0 = 不限）",
  "set.limitTip": "建議參考課程學習習慣中的建議學習時長。",
  "set.tray": "最小化到系統匣",
  "set.trayTip": "開啟時關閉視窗縮到系統匣，背景繼續刷課；關閉時點 × 直接結束程式。",
  "set.dark": "深色模式",
  "set.darkTip": "深色介面，夜間刷課更護眼，立即生效。",
  "set.lang": "介面語言",
  "set.langTip": "切換後立即生效，無需儲存。",

  "login.slogan": "把網課丟給它，你去吃點好的",
  "login.tabQr": "掃碼登入（推薦）",
  "login.tabBrowser": "瀏覽器登入",
  "login.qrAlt": "登入 QR Code",
  "login.qrFetching": "正在取得 QR Code…",
  "login.qrScanned": "✓ 已掃碼<br />確認後正在登入…",
  "login.qrHint": "📱 打開<b>知到 APP</b>，掃描上面的 QR Code",
  "login.qrRefresh": "重新整理 QR Code",
  "login.browserHint":
    "點下方按鈕後，本視窗會變成智慧樹登入頁面。<br />在頁面裡輸入帳號密碼（需要滑塊/簡訊驗證也直接完成），<br /><b>登入成功後本程式會自動返回並進入主介面</b>。",
  "login.openBrowser": "打開登入頁面",
  "login.opening": "正在打開…",
  "login.browserTip": "推薦使用「掃碼登入」，無需輸入帳號密碼更安全。",
  "login.cancelled": "已在手機上取消登入，請點下方按鈕重新整理 QR Code 後重試",
  "login.expired": "QR Code 已過期，正在自動重新整理…",
  "login.retry": "（點此重試）",
  "login.netRetry": "（請檢查網路後點下方按鈕重試）",
  "login.footer":
    "僅供學習交流 · 請合理安排時間 · 本工具不會上傳任何資料到第三方",

  "boot": "正在啟動…",
};

const en: Record<string, string> = {
  "nav.courses": "My Courses",
  "nav.run": "Progress",
  "nav.settings": "Settings",
  "side.logout": "↩ Log out",

  "btn.refresh": "Refresh courses",
  "btn.refreshing": "Loading…",
  "pickAll": "Select all",
  "btn.start": "Start ({0} courses)",
  "type.share": "Shared",
  "type.school": "School",
  "tag.running": "Running",
  "tag.queued": "Queued",
  "dragTip": "Drag the handle next to a course name to reorder",
  "empty.courses.title": "No courses found 😢",
  "empty.courses.sub":
    "Possible reasons: no courses on this account, session expired, or network issues. Click refresh above.",
  "err.loadCourses": "Failed to load courses: {0}",
  "err.start": "Failed to start: {0}",
  "err.startDup": "{0} course(s) already in the queue — not added again",

  "run.empty.title": "Nothing running yet",
  "run.empty.sub":
    'Pick courses in "My Courses" and click Start. Closing the window keeps studying in the background.',
  "run.section.active": "Studying now",
  "run.section.queue": "Queued ({0})",
  "run.section.done": "Done / Stopped",
  "btn.pause": "⏸ Pause",
  "btn.resume": "▶ Resume",
  "btn.clearAll": "🗑 Clear all",
  "btn.skip": "Skip this course",
  "btn.remove": "Remove",
  "run.pausedTip": "Paused: video progress is frozen. Click Resume to continue.",
  "notice.captchaCleared":
    "Your account was temporarily restricted by Zhihuishu — the remaining queued courses were cleared. Complete a manual verification, then start again.",
  "state.paused": "Paused",
  "state.simPause": "Simulated pause",
  "state.quiz": "Answering quiz…",
  "run.preparing": "Preparing…",
  "tag.done": "Done",
  "tag.stopped": "Stopped",
  "tag.skipped": "Skipped",
  "tag.captcha": "Needs verification",
  "tag.limit": "Time limit reached",
  "tag.error": "Error",
  "captcha.title":
    "⚠️ Your account was temporarily restricted by Zhihuishu. Studying is paused (the app is not broken).",
  "captcha.intro":
    "After a restriction there is a 30–60 minute account cooldown — during it every retry fails again, and each failure deepens the restriction. Follow the steps below in order; don't skip ahead and don't retry early:",
  "captcha.s1":
    "① Wait 30–60 minutes first (do NOT click restart during this — the earlier you retry, the worse it gets)",
  "captcha.s2":
    "② Log in to the Zhihuishu website in a browser and manually play any video for 1–2 minutes",
  "captcha.s3": "③ Come back here, click \"Log out\" at the bottom left, then scan the QR code to log in again",
  "captcha.s4": "④ Run only ONE course first; add the next after it finishes",
  "captcha.retry": "I've completed the verification — restart",
  "confirm.logout":
    "Tasks are still running. Logging out will stop them all. Continue?",
  "confirm.clearAll": "This cannot be undone. Delete everything?",
  "confirm.cooldown":
    "Your account was restricted {0} minute(s) ago and is likely still in the risk-control cooldown (about 30–60 minutes).\n\nRetrying now will most likely fail again within minutes — and every failure deepens the restriction and extends the cooldown.\n\nRecommended: wait 30–60 minutes → log in to the Zhihuishu website and manually play a video for 1–2 minutes → log in again via QR code → run only ONE course. Retry anyway?",
  "dlg.ok": "OK",
  "dlg.cancel": "Cancel",

  "log.taskStart": "Task started",
  "log.loading": "Loading course {0} …",
  "log.startCourse": "Studying: {0}",
  "log.chapter": "Chapter: {0}",
  "log.videoStart": "Watching: {0}",
  "log.videoDone": "Finished: {0}",
  "log.quizAnswered": "Auto-answered: {0}",
  "log.skipped": "Skipped {0}: {1}",
  "reason.watched": "already watched",
  "reason.quiz": "quiz — auto-completion not supported",
  "reason.resource": "non-video resource, visited",
  "log.limit": "Course time limit reached — this course stopped",
  "log.captchaHit": "Account restricted by Zhihuishu — task paused (see above)",
  "log.manualStop": "Stopped manually",
  "done.courseFinished": "Course finished",
  "done.finished": "Task finished",
  "done.stopped": "Task stopped",
  "done.captcha":
    "Task paused: account restricted by Zhihuishu (see the notice above)",

  "set.speed": "Playback speed",
  "set.speedAuto":
    "Platform max (recommended; 1.5x shared / 1.25x school courses)",
  "set.speed25": "2.5x (fast, slightly riskier)",
  "set.speed30": "3.0x (extreme, high risk)",
  "set.speedTip":
    "The default is the safest choice. Higher speed finishes faster but is more likely to get flagged by Zhihuishu.",
  "set.threshold": "Completion threshold: {0}%",
  "set.thresholdTip":
    "A video counts as finished once playback reaches this percentage. Default 100% watches the whole video.",
  "set.limit": "Per-course time limit (minutes, 0 = unlimited)",
  "set.limitTip": "A good reference is the suggested study time in your course's study habits.",
  "set.tray": "Minimize to tray",
  "set.trayTip":
    "On: closing the window minimizes to the tray and keeps studying. Off: closing the window quits the app.",
  "set.dark": "Dark mode",
  "set.darkTip": "Dark interface — easier on the eyes at night. Applies immediately.",
  "set.lang": "Language",
  "set.langTip": "Applies immediately — no need to save.",

  "login.slogan": "Leave the online classes to me — go grab a snack",
  "login.tabQr": "QR code (recommended)",
  "login.tabBrowser": "Browser login",
  "login.qrAlt": "Login QR code",
  "login.qrFetching": "Fetching QR code…",
  "login.qrScanned": "✓ Scanned<br />Confirming on your phone…",
  "login.qrHint": "📱 Open the <b>Zhidao APP</b> and scan the QR code above",
  "login.qrRefresh": "Refresh QR code",
  "login.browserHint":
    "After clicking the button below, this window becomes the Zhihuishu login page.<br />Enter your account and password there (captcha/SMS verification included),<br /><b>the app returns to the main screen automatically once you're logged in</b>.",
  "login.openBrowser": "Open login page",
  "login.opening": "Opening…",
  "login.browserTip":
    "QR code login is recommended — no password needed, and it's safer.",
  "login.cancelled":
    "Login was cancelled on your phone. Click the button below to refresh the QR code and try again.",
  "login.expired": "QR code expired — refreshing automatically…",
  "login.retry": "(click here to retry)",
  "login.netRetry": "(check your network, then click the button below to retry)",
  "login.footer":
    "For study purposes only · Manage your time wisely · No data is ever sent to third parties",

  "boot": "Starting…",
};

const dicts: Record<Lang, Record<string, string>> = {
  "zh-CN": zhCN,
  "zh-TW": zhTW,
  en,
};

/** 取翻译：{0} {1} 占位符按参数顺序替换；缺失时回退简体中文。 */
export function t(key: string, ...args: (string | number)[]): string {
  const d = dicts[lang.value] ?? zhCN;
  let s = d[key] ?? zhCN[key] ?? key;
  args.forEach((a, i) => {
    s = s.split(`{${i}}`).join(String(a));
  });
  return s;
}
