// 生成应用图标源图：青绿渐变圆角方块 + 白「知」+ 红色禁制斜杠（anti 语义）
// 用法: node scripts/make-icon.mjs  ->  app-icon.png (1024x1024)
// 之后运行: npx tauri icon app-icon.png 生成全套尺寸
import fs from "node:fs";
import { Resvg } from "@resvg/resvg-js";

const svg = `<svg width="1024" height="1024" xmlns="http://www.w3.org/2000/svg">
  <defs>
    <linearGradient id="g" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="#27CBA9"/>
      <stop offset="1" stop-color="#0A8F80"/>
    </linearGradient>
  </defs>
  <rect width="1024" height="1024" rx="230" fill="url(#g)"/>
  <text x="512" y="710" text-anchor="middle" fill="#ffffff"
        font-family="Microsoft YaHei" font-weight="bold" font-size="530">知</text>
  <!-- 禁制斜杠：白边打底分隔字形，红杠在上 -->
  <line x1="196" y1="196" x2="828" y2="828" stroke="#ffffff" stroke-width="104" stroke-linecap="round"/>
  <line x1="196" y1="196" x2="828" y2="828" stroke="#E0483C" stroke-width="64" stroke-linecap="round"/>
</svg>`;

const resvg = new Resvg(svg, {
  fitTo: { mode: "width", value: 1024 },
  font: {
    fontFiles: ["C:\\Windows\\Fonts\\msyhbd.ttc"],
    loadSystemFonts: false,
    defaultFontFamily: "Microsoft YaHei",
  },
});
fs.writeFileSync("app-icon.png", resvg.render().asPng());

// 小尺寸预览，自查任务栏可读性
const small = new Resvg(svg, {
  fitTo: { mode: "width", value: 48 },
  font: {
    fontFiles: ["C:\\Windows\\Fonts\\msyhbd.ttc"],
    loadSystemFonts: false,
    defaultFontFamily: "Microsoft YaHei",
  },
});
fs.writeFileSync("app-icon-preview-48.png", small.render().asPng());
console.log("app-icon.png + preview written");
