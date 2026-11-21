// 应用内弹窗（非系统弹窗）：圆角矩形卡片，跟随亮色/暗色主题。
// 背景：Tauri 的 WebView2 里 JS 原生 alert()/confirm() 是空操作；系统原生弹窗
// 又是 Windows 风格、不吃应用主题——所以自绘：reactive 状态 + AppDialog.vue 渲染。
import { reactive } from "vue";

export interface DialogOptions {
  /** 确定按钮标红（危险操作，如清空全部） */
  danger?: boolean;
}

interface DialogState {
  open: boolean;
  message: string;
  /** confirm = 确定+取消 两个按钮；alert = 只有确定 */
  mode: "confirm" | "alert";
  danger: boolean;
  resolve: ((v: boolean) => void) | null;
}

export const dialogState = reactive<DialogState>({
  open: false,
  message: "",
  mode: "alert",
  danger: false,
  resolve: null,
});

function open(
  message: string,
  mode: "confirm" | "alert",
  opts?: DialogOptions
): Promise<boolean> {
  // 已有弹窗未关闭时，前一个按"取消"结算
  if (dialogState.resolve) dialogState.resolve(false);
  dialogState.message = message;
  dialogState.mode = mode;
  dialogState.danger = opts?.danger ?? false;
  dialogState.open = true;
  return new Promise<boolean>((resolve) => {
    dialogState.resolve = resolve;
  });
}

/** 应用内确认弹窗：确定返回 true，取消/Esc/点遮罩返回 false。 */
export function uiConfirm(msg: string, opts?: DialogOptions): Promise<boolean> {
  return open(msg, "confirm", opts);
}

/** 应用内提示弹窗：只有确定按钮。 */
export function uiAlert(msg: string, opts?: DialogOptions): Promise<void> {
  return open(msg, "alert", opts).then(() => undefined);
}

/** 由 AppDialog 调用：用户点了确定/取消，结算并关闭。 */
export function settleDialog(result: boolean) {
  if (!dialogState.resolve) return;
  const resolve = dialogState.resolve;
  dialogState.resolve = null;
  dialogState.open = false;
  resolve(result);
}
