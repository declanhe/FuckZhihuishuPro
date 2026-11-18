# FuckZhihuishuPro

> 智慧树（知到）刷课助手 —— 集 [fuckZHS](https://github.com/VermiIIi0n/fuckZHS) 与 [Autovisor](https://github.com/CXRunfree/Autovisor) 之大成。

**Rust + Tauri 原生实现，无需 Python、无需浏览器，双击即用。**

## 为什么是它

| 特性 | fuckZHS | Autovisor | **FuckZhihuishuPro** |
| --- | --- | --- | --- |
| 运行方式 | 终端命令行 | 浏览器自动化（Playwright） | **图形界面客户端** |
| 依赖 | Python 3.10+ + pip | 内嵌 Python + Chrome/Edge | **无任何依赖，单 exe** |
| 登录 | 终端二维码（字符画） | 浏览器内手动/滑块 | **窗口内二维码 / 账号密码** |
| 倍速 | 命令行参数 | 手动改 config.ini | **默认平台最高，可在设置调整** |
| 后台运行 | 终端挂着 | 依赖浏览器存活 | **托盘后台，关窗继续刷** |
| 弹题 | 自动作答 | 自动作答 | **自动作答（无需 AI）** |
| 核心原理 | 纯 HTTP API | 操控真实浏览器 | 纯 HTTP API（fuckZHS 协议移植） |

## 功能

- ✅ 扫码登录（知到 APP 扫码，窗口内展示二维码）/ 账号密码登录
- ✅ 登录态持久化：短时间内打开程序免扫码
- ✅ 知到共享学分课 + 校内学分课（hike）自动刷课
- ✅ 课中弹题自动作答（答案直接取自题目数据，不需要配置任何 AI）
- ✅ 默认平台最高倍速（共享课 1.5x / 校内课 1.25x），设置里可调更高（有风控风险）
- ✅ 关闭窗口最小化到托盘，后台静默继续刷课；退出请在托盘右键菜单操作
- ✅ 实时进度条 + 运行日志，随时手动停止任意课程
- ✅ 完成阈值 / 单课时限可配置

## 使用方法

1. 双击启动（开发阶段 `npm run tauri dev`；发布版直接双击 `FuckZhihuishuPro.exe`）
2. 用**知到 APP** 扫窗口里的二维码，或在"账号登录"页输入账号密码
3. 在"我的课程"勾选要刷的课程 → 点 **开始刷课**
4. "刷课进度"页可实时看到每门课的章节 / 视频 / 进度条 / 弹题作答记录
5. 想干别的？直接关窗口，程序缩到托盘继续刷；托盘左键点图标恢复窗口，右键可退出

> ⚠️ 如果账号在非常用设备登录需要短信验证：先用浏览器登录一次智慧树网页版，之后再用本工具。

## 开发

```bash
# 环境：Node 18+、Rust stable-msvc、VS Build Tools（C++ 工作负载）、WebView2 Runtime（Win11 自带）
npm install
npm run tauri dev      # 开发调试
npm run tauri build    # 打包出 exe + 安装包（src-tauri/target/release/）
```

### 项目结构

```
src-tauri/src/
├── core/                 # 核心引擎（fuckZHS 协议的 Rust 移植）
│   ├── crypto.rs         # AES-128-CBC secretStr / ev 混淆编码 / MD5 签名 / WatchPoint
│   │                     #   ↔ zd_utils.py + sign.py（含与原版 Python 的交叉验证测试）
│   ├── cookies.rs        # 可导出的 Cookie 存储（↔ requests.Session）
│   ├── session.rs        # HTTP 会话与 API 封装（↔ apiQuery / zhidaoQuery / hikeQuery）
│   ├── login.rs          # 密码登录 + 扫码登录（↔ login / _qrlogin）
│   ├── zhidao.rs         # 共享学分课（↔ getZhidaoContext / fuckZhidaoCourse / 弹题）
│   ├── hike.rs           # 校内学分课（↔ getHikeList / fuckHikeCourse / saveStuStudyRecord）
│   ├── engine.rs         # 任务调度：启动 / 停止 / 事件转发
│   └── events.rs         # 进度事件 + 停止令牌
├── commands.rs           # Tauri 命令层（前端 invoke 的接口）
├── config.rs             # 配置持久化（倍速 / 阈值 / 时限）
└── lib.rs                # 应用入口（托盘 / 关窗后台）

src/                      # 前端（Vue 3 + TS）
├── api.ts                # invoke 封装
├── App.vue               # 登录态路由
└── components/
    ├── Login.vue         # 扫码 / 账密登录
    └── Home.vue          # 课程列表 / 刷课进度 / 设置
```

### 技术要点

- 进度上报的 ev 混淆编码、AES-CBC secretStr、MD5 签名、WatchPoint 观看点模拟，全部 1:1 移植自 fuckZHS，并通过原版 Python 实现的交叉验证
- 模拟人类行为：随机暂停、随机收尾延时，行为特征与 fuckZHS 一致
- 全异步（tokio）：多门课程并行刷，互不阻塞

## 免责声明

本项目仅供学习交流使用，请尊重学习本身——工具只是帮你把重复劳动省下来，该学的东西还是要学的。使用本工具产生的一切后果由使用者自行承担。

## 致谢

- [fuckZHS](https://github.com/VermiIIi0n/fuckZHS) —— 协议逆向与核心逻辑来源
- [Autovisor](https://github.com/CXRunfree/Autovisor) —— 面向大众易用性的产品思路
