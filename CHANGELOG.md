# Changelog

All notable changes to this project will be documented in this file.

## [2.1.0] — 2026-10-22

• Enhanced Non-intrusive Canvas Experience: Fixed an issue where Photoshop shortcuts could become unresponsive in Turbo Mode, ensuring seamless shortcut usage while the bar is hovering.

• Universal macOS Support: macOS release is now unified into a single Universal package, natively supporting both Apple Silicon and Intel Macs.

• 极致无干扰画布体验：修复了在 Turbo 模式下悬浮工具栏可能导致 Photoshop 快捷键失灵的问题，保证创作过程中快捷键随时可用。

• macOS 通用架构支持：macOS 安装包升级为 Universal 通用架构，一套安装包无缝原生支持 Apple Silicon 及 Intel 芯片 Mac。

## [2.0.0] — 2026-10-01

• Architecture Decoupling: Turbo Bar is now a pure dumb widget renderer. All icon SVGs and visual styling are dynamically passed via WebSocket protocol, eliminating the need to rebuild the client for icon adjustments.

• Crop Layer Bounds Support: Added support for `cropLayer` button alongside `cropSelection`.

• 架构解耦：Turbo Bar 升级为纯无状态渲染器，按钮图标（SVG）完全由 WebSocket 协议动态下发，后续修改或扩展图标不再需要重新编译客户端。

• 支持按图层裁剪：新增对 `cropLayer`（按图层边界裁剪）按钮的支持，并支持与选区裁剪协同切换。

## [1.0.0] — 2026-09-19

🎉 Initial Release

• First public release of ComfyPanel Turbo Bar.

• Introduces a lightweight, frameless interface for ComfyPanel Turbo Mode.

• Designed as a companion component for the Turbo / Studio experience introduced in ComfyPanel 2.0.

• Distributed as an independent component for macOS and Windows.

🎉 首次发布

• ComfyPanel Turbo Bar 首次公开发布。

• 为 ComfyPanel Turbo Mode 提供轻量、无边框的操作界面。

• 作为 ComfyPanel 2.0 全新 Turbo / Studio 体验的配套组件。

• 作为独立组件发布，支持 macOS 和 Windows。