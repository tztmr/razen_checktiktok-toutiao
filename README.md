# iOS Sandbox ZIP Reader

这是一个桌面端原型项目，用来读取“整理后的 iOS APP 沙盒 ZIP 包”，并在本地解析常见参数文件。

当前技术路线：

- 桌面框架：`Tauri 2`
- 前端：`React + TypeScript`
- 核心解析：`Rust`
- 目标平台：`macOS`、`Windows`

## 第一版范围

第一版 MVP 聚焦三类文件：

- `plist`
- `json`
- `sqlite`

当前原型能力：

- 输入单个 `zip` 路径
- 扫描 zip 内 APP 列表
- 列出候选参数文件
- 解析 `plist/json/sqlite`
- 在界面中预览结构化结果

## 相关文档

- 方案概览：[`README.md` 当前文件]
- 第二版研究方案：[`V2_TAURI_RUST_RESEARCH.md`](file:///Users/edking/Documents/%E7%BD%91%E8%B5%9A%E5%AD%A6%E4%B9%A0/ios_zen_plist_read/V2_TAURI_RUST_RESEARCH.md)

## 开发命令

安装依赖：

```bash
npm install
```

启动前端开发：

```bash
npm run dev
```

启动 Tauri 桌面开发：

```bash
npm run tauri dev
```

## 当前说明

你之前提供的样本路径可直接用于第一版测试：

```text
/Users/edking/Desktop/11/20260505-12-24-20_67080761757.zip
```

后续建议优先继续做：

1. 第二版先补导出和缓存
2. 再增强 `sqlite` 结果展示
3. 再推进 `mmkv` 第二阶段研究
4. 最后接 GitHub Actions 双平台打包
