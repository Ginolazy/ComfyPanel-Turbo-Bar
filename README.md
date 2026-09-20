# ComfyPanel Turbo Bar

**Turbo Bar is a lightweight floating component of ComfyPanel**, designed to provide a faster and more focused way to work with AI in Photoshop.

It keeps essential actions close at hand without taking up space in the main ComfyPanel interface.

## Features

* **Minimal** — A clean, compact floating bar.
* **Focused** — Keeps the interface centered on the current task.
* **Flexible** — Adapts to different workflows and situations.
* **Seamless** — Works naturally alongside the main ComfyPanel.
* **Fast** — Quickly switch between Studio and Turbo modes.

## Turbo Mode

Turbo Bar is available when **Turbo Mode** is active.

Use **Studio Mode** when you need the full ComfyPanel interface, including detailed controls and adjustments.

Switch to **Turbo Mode** when you want a simpler, more focused workspace for quick operations.

```text
Studio Mode
Full workspace

      ↕

Turbo Mode
Focused workspace
```

## First Launch

When using Turbo Bar for the first time, Photoshop may display a permission dialog for `comfypanel-turbo-bar://helper`.

Please **check “Remember my choice”** and allow the requested permission.

This only needs to be confirmed during the initial setup.

### macOS Developer Verification Warning

If macOS says that “ComfyPanel Turbo Bar.app” cannot be opened because the developer cannot be verified,
run this command in Terminal:

```bash
xattr -d com.apple.quarantine "/Applications/ComfyPanel Turbo Bar.app"
```

## Requirements

* **ComfyPanel 2.0 or later**
* Adobe Photoshop 2025 or later

## Part of ComfyPanel

Turbo Bar is a component of **ComfyPanel** and does not require separate configuration.

For installation and usage, please refer to the ComfyPanel documentation.

---

# ComfyPanel Turbo Bar（中文）

**Turbo Bar 是 ComfyPanel 的一个轻量级浮动组件**，为 Photoshop 中的 AI 创作提供更加快速、专注的操作方式。

它将常用操作保持在触手可及的位置，同时不占用 ComfyPanel 主界面的空间。

## 特点

* **极简** — 简洁、紧凑的浮动界面。
* **专注** — 围绕当前任务提供更加集中的操作体验。
* **灵活** — 可适应不同的工作流和使用场景。
* **无缝** — 与 ComfyPanel 主界面自然配合。
* **快速** — 在 Studio Mode 和 Turbo Mode 之间快速切换。

## Turbo Mode

Turbo Bar 在 **Turbo Mode** 下使用。

需要完整的 ComfyPanel 界面、详细控制和调参时，可以使用 **Studio Mode**。

需要快速完成当前操作、减少界面干扰时，可以切换到 **Turbo Mode**。

```text
Studio Mode
完整工作区

      ↕

Turbo Mode
专注工作区
```

## 首次使用

首次使用 Turbo Bar 时，Photoshop 可能会弹出针对 `comfypanel-turbo-bar://helper` 的权限请求。

请**勾选“记住我的选择”**，然后允许所请求的权限。

完成首次设置后，通常无需再次确认。

### macOS 开发者验证提示

如果 macOS 提示“ComfyPanel Turbo Bar.app”无法打开，请在终端执行：

```bash
xattr -d com.apple.quarantine "/Applications/ComfyPanel Turbo Bar.app"
```

## 系统要求

* **ComfyPanel 2.0 或更高版本**
* Adobe Photoshop 2025 或更高版本

## ComfyPanel 的一部分

Turbo Bar 是 **ComfyPanel 的一个组件**，无需单独配置。

安装和使用方法请参阅 ComfyPanel 官方文档。

---

© 2026 LAZYet
