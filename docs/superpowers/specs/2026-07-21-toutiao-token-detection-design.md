# 今日头条 Token 检测设计

## 目标

为现有批量检测工作台增加独立的今日头条 Token 检测。检测请求使用 ZIP 内 `com.ss.iphone.article.News` 的真实凭据与设备参数访问 `tabs_api/v1`，根据响应判断 Token 是否在线，并把用户名、UID、注册时间写入现有批量结果列。

## 用户界面

- 在“今日头条检测”卡片中增加独立的 `Token` 复选框，默认开启。
- 保留现有“登录/实名状态”复选框，两个选项互不覆盖、可以分别关闭。
- 点击“开始检测头条”时，只执行用户勾选的检测项。
- Token 成功时，现有表格的“账号”“UID”“注册时间”“Token 状态”列显示 API 返回结果。
- Token 失败时，“Token 状态”区分掉线、缺参数、HTTP 失败、请求失败和解析失败；错误详情保留到现有错误列和详情弹窗。

## 后端组件

新增一个 Tauri 命令 `check_toutiao_token_status`，职责如下：

1. 从目标 ZIP 的头条主 plist 读取：
   - `kTTAccountTokenGuardXTTToken`，回退到 `bdaccount_session_x_tt_token`；
   - `FlowSaveDeviceId.deviceId`，缺失时回退到 `kOldDeviceIDStorageKey`；
   - 头条版本信息，用于构造与包匹配的 User-Agent。
2. 从头条 `Cookies.binarycookies` 读取：
   - `odin_tt`、`store-region`、`store-region-src`；
   - `passport_csrf_token` 和 `passport_csrf_token_default`（存在时携带）；
   - `install_id`，作为请求的 `iid`。
3. 构造固定业务参数：
   - `app_name=news_article`
   - `aid=13`
   - `detail=my_tabs_v2`
   - `user_app_id=1128`
4. 请求 `https://api5-normal-hl.toutiaoapi.com/tabs_api/v1/`，超时 15 秒，不把完整 Token 或 Cookie 返回给前端。
5. 解析 `profile.data`：
   - `name` -> 用户名
   - `user_id` -> UID
   - `create_time` -> 注册时间

## 状态判定

- `ok`：HTTP 成功，顶层 `message` 为 `success`，并且 `profile.errno` 为 `0`、`profile.message` 为 `success`，同时能读取有效的 `profile.data.user_id`。
- `invalid`：服务端明确返回非成功业务状态，或响应表明登录凭据失效。
- `missing_token`、`missing_odin_tt`、`missing_device_id`、`missing_iid`：包内缺少必要参数。
- `http_error`：HTTP 非成功状态。
- `request_error`：超时、DNS、TLS 或其他请求错误。
- `parse_error`：响应不是预期 JSON，或成功响应缺少必要身份字段。

只有 `ok` 可以把整行标记为在线。缺参数和网络错误不得伪装为掉线；若同一行的实名认证检测成功，整行可以保持在线，但 Token 列仍必须显示自己的真实失败状态。

## 前端数据流

`ToutiaoDetectionOptions` 增加 `token`。`buildBatchDetectionOptions` 继续作为平台选项的唯一组装入口，确保抖音和头条选项相互独立。

批量检测头条行时：

1. 若勾选 Token，调用 `check_toutiao_token_status`。
2. 将 `nickname`、`uid`、格式化后的 `create_time` 合并到现有行字段。
3. 根据 Token 结果更新在线/掉线信号与功能说明。
4. 若勾选实名认证，再执行现有 `check_toutiao_certification_status`；两项结果分别展示，行级状态按已有信号合并规则计算。

## 隐私与日志

- 前端只接收 Token 和 `odin_tt` 的掩码预览，不接收完整秘密值。
- CSV、错误文本、详情弹窗和 Rust 错误不得包含完整 Token、Cookie 或请求头。
- 不把用户提供的示例 Cookie、Token、设备 ID 或安装 ID 写入源码和测试快照。

## 测试与验收

- Rust 单元测试覆盖参数选择、查询参数构造、Cookie 白名单、响应成功解析以及各种失败状态。
- TypeScript 测试覆盖头条 Token 选项默认组装和平台选项隔离。
- 前端构建和 Rust 测试全部通过。
- 使用以下两个 ZIP 进行真实检测：
  - `/Users/edking/Downloads/20260610-05-23-53_62995022752.zip`
  - `/Users/edking/Downloads/20260610-05-33-22_29159868055.zip`
- 验证每个 ZIP 都使用自己的包内凭据和设备参数，并核对在线状态、用户名、UID 和注册时间。
- 最后构建并检查 `src-tauri/target/release/bundle/macos/iOS Sandbox ZIP Reader.app`。

## 非目标

- 不保存或上传扫描结果到第三方存储。
- 不尝试刷新、续期或修改头条 Token。
- 不将本接口扩展为通用 HTTP 调试器。
