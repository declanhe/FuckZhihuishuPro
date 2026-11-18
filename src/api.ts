// 后端命令封装 —— 与 src-tauri/src/commands.rs 一一对应
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export interface AppConfig {
  speed: number;
  end_threshold: number;
  limit_minutes: number;
  minimize_to_tray: boolean;
}

export interface CourseItem {
  name: string;
  id: string;
}

export interface CourseEvent {
  kind: string;
  level?: string;
  msg?: string;
  name?: string;
  reason?: string;
  video_id?: string;
  played?: number;
  total?: number;
  paused?: boolean;
  quiz?: boolean;
  question?: string;
}

export interface CourseEventPayload {
  courseId: string;
  courseName: string;
  event: CourseEvent;
}

export const api = {
  getLoginState: () =>
    invoke<{ logged_in: boolean; uuid: string | null }>("get_login_state"),
  loginWithPassword: (username: string, password: string) =>
    invoke<void>("login_with_password", { username, password }),
  qrLoginStart: () =>
    invoke<{ qrToken: string; img: string }>("qr_login_start"),
  qrLoginPoll: (qrToken: string) =>
    invoke<{ status: string; msg?: string }>("qr_login_poll", { qrToken }),
  logout: () => invoke<void>("logout"),
  restoreLogin: () => invoke<boolean>("restore_login"),
  openBrowserLogin: () => invoke<void>("open_browser_login"),
  getCourses: () =>
    invoke<{ zhidao: CourseItem[]; hike: CourseItem[] }>("get_courses"),
  startCourses: (courses: CourseItem[]) =>
    invoke<string[]>("start_courses", { courses }),
  stopCourse: (courseId: string) => invoke<boolean>("stop_course", { courseId }),
  stopAll: () => invoke<void>("stop_all_courses"),
  getRunning: () => invoke<string[]>("get_running"),
  getConfig: () => invoke<AppConfig>("get_config"),
  saveConfig: (config: AppConfig) => invoke<void>("save_config", { config }),
  getDataDir: () => invoke<string>("get_data_dir"),
  selfTest: () => invoke<boolean>("self_test"),
};

/** 监听刷课进度事件，返回取消监听函数。 */
export function onCourseEvent(
  handler: (payload: CourseEventPayload) => void
): Promise<() => void> {
  return listen<CourseEventPayload>("course-event", (e) => handler(e.payload));
}

/** 把错误 JSON 字符串还原为对象。 */
export function parseErr(err: unknown): { kind?: string; message: string } {
  if (typeof err === "string") {
    try {
      return JSON.parse(err);
    } catch {
      return { message: err };
    }
  }
  return { message: String(err) };
}
