# 跨屏收窗 · AutoWindowAdjust

把窗口拖到另一块屏幕，松手后自动收好。**放得下就保留大小，放不下才按比例缩小。**

[**下载 Windows 版**](https://github.com/CosPie/auto-window-adjust/releases/latest) · [快速开始](#快速开始) · [使用边界](#使用边界) · [开发与验证](#开发与验证)

<p>
  <img src="https://img.shields.io/badge/Windows-10%20%2F%2011-087F79?style=flat-square" alt="Windows 10 / 11">
  <img src="https://img.shields.io/badge/x64-59717A?style=flat-square" alt="x64">
  <a href="./LICENSE"><img src="https://img.shields.io/badge/License-MIT-59717A?style=flat-square" alt="MIT License"></a>
</p>

<p align="center">
  <img src="./assets/readme/window-fit.svg" width="100%" alt="行为示意：把横屏上的窗口拖到竖屏，松手后窗口缩小并收进竖屏的可用区域。">
</p>

这是一个 Windows 托盘小工具，适合在横屏、竖屏或不同尺寸的显示器之间频繁移动窗口的人。保留原来的鼠标拖拽习惯，不需要先设置分区或记住快捷键。

## 自动调整什么？

| 拖动后的情况 | 工具的处理 |
| --- | --- |
| 窗口放得下，但一部分留在另一块屏幕 | 只调整位置，保留大小 |
| 窗口比目标屏幕可用区域更大 | 按比例缩小，再收进屏幕，避开任务栏 |
| 想故意让窗口跨两块屏幕 | 按住 **Alt** 松手，跳过本次调整 |

以**松手时鼠标所在的屏幕**为目标。同一屏幕内拖动、拖边框缩放、最大化和最小化窗口不处理。

## 快速开始

1. 打开 [Releases](https://github.com/CosPie/auto-window-adjust/releases/latest)，下载 **AutoWindowAdjust.exe**。
2. 把文件放到固定目录，双击运行。免安装，无需另外安装 .NET。
3. 将一个普通窗口拖到另一块屏幕，松开鼠标即可。

启动后没有主窗口，会出现托盘提示。图标可能藏在任务栏右下角的 **「显示隐藏的图标」** 中。

| 操作 | 方法 |
| --- | --- |
| 暂停 / 恢复 | 双击托盘图标，或右键菜单切换 |
| 临时跳过一次 | 按住 **Alt** 松手 |
| 开机启动 | 右键托盘图标 → **开机启动**，默认关闭 |
| 完全停止 | 右键托盘图标 → **退出** |

移动 EXE 的位置后，请重新勾选开机启动以更新路径。卸载时取消开机启动、退出工具，再删除 EXE 即可。

## 使用边界

- **应用有最小尺寸限制时**，窗口可能无法完全缩进更小的屏幕；工具会尽量保留可操作的左上角。
- **管理员应用**可能无法由普通权限的工具调整。需要处理这些窗口时，可手动以管理员身份运行本工具。
- **部分自绘窗口**不触发 Windows 标准移动事件，可能无法识别。
- **拖回大屏不会自动放大**：当前版本不记忆窗口在各块屏幕上的历史尺寸。
- 跨屏松手后等待约 **180ms** 再调整，让应用先处理缩放和贴靠变化。最大化状态由 Windows 处理；工具不接管系统贴靠布局。

提供 Windows x64 版本。已在本机的横屏 + 竖屏组合上验证，其他显示器排列和应用组合欢迎通过 [Issues](https://github.com/CosPie/auto-window-adjust/issues) 反馈。

## 隐私与运行方式

程序不联网、不收集窗口标题或内容，也不向其他进程注入代码。它通过 Windows 事件监听窗口拖动的开始与结束，无需持续轮询窗口。

只有开启开机启动时，才会在当前用户注册表写入启动项：

```text
HKCU\Software\Microsoft\Windows\CurrentVersion\Run\AutoWindowAdjust
```

## 开发与验证

需要 Windows 和 **.NET 8 SDK 或更新版本**。

```powershell
git clone https://github.com/CosPie/auto-window-adjust.git
cd auto-window-adjust
./build.ps1
```

脚本先运行验证，再生成 `dist/AutoWindowAdjust.exe`，产物自带运行时。首次发布需要联网下载 .NET 运行时包。

仅运行验证：

```powershell
dotnet run --project tests/AutoWindowAdjust.Tests.csproj -c Release
```

验证包含 11 个几何计算用例，覆盖横竖屏、负坐标、任务栏和固定尺寸；还会在当前每块显示器上创建临时窗口，检查实际 Win32 调整和重复调整的稳定性。原生窗口验证需要可交互的 Windows 桌面。

提交改动前，建议手动检查跨屏双向拖动、不同缩放比例、Alt 跳过、同屏拖动、边框缩放、最大化/贴靠以及托盘暂停与退出。

<details>
<summary>实现概览</summary>

使用 C# / WinForms 和 Win32 API：

- `SetWinEventHook`：监听移动 / 缩放开始和结束。
- `MonitorFromPoint` / `GetMonitorInfo`：确定目标屏幕与可用区域。
- Per-Monitor V2 DPI 感知和 DWM 可见边框：计算窗口的实际显示范围。
- `SetWindowPos`：调整窗口大小与位置。

核心几何计算位于 [WindowGeometry.cs](./WindowGeometry.cs)，事件处理位于 [WindowWatcher.cs](./WindowWatcher.cs)，托盘入口位于 [TrayContext.cs](./TrayContext.cs)。

</details>

## 反馈与贡献

欢迎提交 [Issue](https://github.com/CosPie/auto-window-adjust/issues) 或 Pull Request。窗口适配问题请附上 Windows 版本、显示器排列、各屏幕的分辨率与缩放比例、应用名称，以及复现步骤。

## 许可证

[MIT](./LICENSE) © 2026 CosPie
