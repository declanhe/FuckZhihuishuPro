# FuckZhihuishuPro

> **把网课丢给它，你去吃点好的。**

智慧树（知到）刷课助手：图形界面客户端，免 Python、免浏览器、双击即用。
纯 HTTP 协议实现，关掉窗口也能在托盘里安静刷课。

## 为什么做它

市面上刷课工具要么要装 Python 配依赖，要么要开个浏览器挂着，
命令行操作对不熟悉技术的同学太不友好。FuckZhihuishuPro 把「配置环境、
扫码登录、选课、防误关」全部装进一个图形界面里：

| | 终端工具 | 浏览器自动化 | **FuckZhihuishuPro** |
| --- | --- | --- | --- |
| 运行方式 | 命令行参数 | 开浏览器挂网页 | **双击即用的客户端** |
| 依赖 | Python 3.10+ | 内嵌 Python + Chrome/Edge | **无任何依赖** |
| 登录 | 终端字符画二维码 | 网页里手动操作 | **窗口内扫码 / 账密** |
| 后台运行 | 终端挂着 | 依赖浏览器存活 | **托盘后台，关窗继续刷** |
| 配置 | 改命令行/配置文件 | 手动设浏览器路径 | **默认即最优，零配置** |

## 功能特性

- ✅ 扫码登录（知到 APP 扫一扫）/ 账号密码登录 / 内嵌浏览器登录（Windows）
- ✅ 登录态持久化，短时间内打开程序免重复登录
- ✅ 共享学分课 + 校内学分课自动刷课
- ✅ 课中弹题自动作答（答案直接取自题目数据，无需配置任何 AI）
- ✅ 默认平台最高倍速（共享课 1.5x / 校内课 1.25x），最高可调 1.8x
- ✅ 实时进度条 + 运行日志（`0:01:00 / 0:10:27（9.6%）`，每秒刷新）
- ✅ 关闭窗口最小化到托盘，后台继续刷课；托盘右键退出
- ✅ 账号触发学习验证时，给出清晰的分步恢复引导
- ✅ 深色模式

## 下载安装

到 [Releases](https://github.com/declanhe/FuckZhihuishuPro/releases) 页面下载：

| 平台 | 文件 | 说明 |
| --- | --- | --- |
| Windows | `FuckZhihuishuPro-vX.Y.Z-portable.zip` | 免安装，解压即用（推荐发同学用这个） |
| Windows | `FuckZhihuishuPro_X.Y.Z_x64-setup.exe` | 安装版 |
| macOS | `FuckZhihuishuPro_X.Y.Z_universal.dmg` | 同时支持 Apple Silicon 和 Intel 芯片 |

- **Windows 10（较新版本）/ Windows 11**：双击即用，无需安装任何依赖
- **macOS**：未签名应用，首次打开请右键 → 打开；登录请用扫码（"浏览器登录"仅 Windows 提供）
- 如果 Windows 弹出"已保护你的电脑"：点"更多信息" → "仍要运行"（程序未购买数字签名）
- 用 QQ / 微信传文件请先打包压缩，直接发 exe 可能被拦截或改名

## 使用方法

1. 双击启动
2. 用**知到 APP** 扫窗口里的二维码（推荐），或切换到账号密码 / 浏览器登录
3. 在"我的课程"勾选要刷的课程 → 点"开始刷课"
4. 在"刷课进度"页实时查看每门课的章节、视频进度条和自动答题记录
5. 想去干别的？直接关窗口，程序缩到屏幕右下角托盘继续刷；
   托盘左键点图标恢复窗口，右键菜单退出

> ⚠️ 如果账号在非常用设备登录需要短信验证：先用浏览器登录一次智慧树网页版，之后再用本工具。

## 倍速与账号安全

- 默认"平台最高"就是最稳的选择，无需任何设置
- 倍速越高刷得越快，被智慧树盯上的概率也越大；1.8x 属于超速上报，请自行权衡
- 如果提示"账号被临时限制"：软件会弹出提示框，跟着里面的四个步骤
  （等待冷却 → 网页版人工验证 → 重新登录 → 先只挂一门课）即可恢复

## 从源码构建

```bash
# 环境：Node 18+、Rust stable、VS Build Tools（Windows）/ Xcode（macOS）
npm install            # 安装前端依赖（建议 npmmirror）
npm run tauri dev      # 开发调试
npm run tauri build    # 构建产物（src-tauri/target/release/bundle/）
```

- 换应用图标：`node scripts/make-icon.mjs && npx tauri icon app-icon.png`
- macOS 无法在 Windows 上交叉编译：打 `v*` 标签推送后，CI 会自动
  构建双平台产物并发布到 Releases
- 重新打便携 zip：`release/pack.ps1`（Windows）

### 项目结构

```
src-tauri/src/
├── core/                 # 核心引擎（纯 HTTP 协议实现）
│   ├── crypto.rs         # AES-128-CBC secretStr / ev 混淆编码 / MD5 签名 / WatchPoint
│   ├── cookies.rs        # 可导出的 Cookie 存储（等价 requests.Session）
│   ├── session.rs        # HTTP 会话与三类 API 查询封装
│   ├── login.rs          # 密码登录 + 扫码登录协议
│   ├── zhidao.rs         # 共享学分课（选课上下文 / 视频模拟 / 弹题）
│   ├── hike.rs           # 校内学分课（课程列表 / 观看记录上报）
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

- 与官方网页客户端一致的 AES-CBC secretStr 加密、MD5 签名、ev 混淆编码，
  观看进度通过 WatchPoint 模拟自然观看轨迹
- 模拟人类行为：随机暂停、随机收尾延时，降低行为特征识别风险
- 全异步（tokio）：多门课程并行刷，互不阻塞；进度上报失败自动降级重试，
  触发风控立即停止任务保护账号
- Windows 上的"浏览器登录"通过 WebView2 CookieManager 复用网页登录会话

## 免责声明

本项目仅供学习交流使用，请尊重学习本身——工具只是帮你把重复劳动省下来，
该学的东西还是要学的。使用本工具产生的一切后果由使用者自行承担。
