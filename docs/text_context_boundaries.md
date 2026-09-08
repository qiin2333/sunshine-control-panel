# Windows 文本上下文：保守收尾

UIA/InputPane 是尽力提供的上下文，不是用户编辑意图或触摸键盘实际弹出的通用证明。

## 已确认的问题与处理

本机 Chrome 网页空白复现：焦点为 Document，TextEditPattern 获取成功，但 Value.IsReadOnly=true。旧 OR 判定把整页标记为 editable，客户端又将页面内点击关联为激活。同期 InputPane Location 一直为空，因此不能将该次误弹归因于主机触摸键盘。

本版移除 TextEditPattern 的查询、缓存和放行作用。只有焦点/启用/非离屏/有效矩形检查通过，且类型为 Edit、Value.IsReadOnly 明确为 false，才报告 editable=true。缺失或失败按未知处理，不报告可编辑。单独的生产纯策略函数包含五项回归测试。

这不是完整 UIA 分类器：Document、富文本、contenteditable、自绘或无 ValuePattern 的编辑器可能漏报。我们主动接受覆盖收缩，不新增应用名单、光标启发式或协议字段。密码控件只读取属性，不读取文本内容。

## 客户端与发布边界

- Moonlight V+ #595 停止依据主机事件自动弹出 IME，保留手动键盘和尽力避让。应配合该客户端更新。
- 此 GUI 修复不禁用 InputPane 通道；旧客户端仍可能依据 InputPane 或明确可写 Edit 自动弹出，不能宣称所有旧客户端已止血。
- 主机 core 的缓存新鲜度、元素身份关联、InputPane 各布局可见性仍不具有全覆盖保证。本次不扩展这些能力。
- 需由 foundation-sunshine 更新 GUI 子模块并重新构建/发布；仅合并 GUI 源码不会改变用户已安装的主机程序。
- 不自动操作或重新配置当前用户的 Windows 触摸键盘。

## 验证边界

回归覆盖可写 Edit、只读 Edit、未知 Edit、Chrome 只读 Document、其他 Document/Custom。历史 Chrome 实测样本作为回归输入，不等同于本版全应用实测；完整 UIA 与真实客户端/主机联调仍需发布前验收。

2026-09-08：直接对生产 `text_context_policy.rs` 执行 `rustc --test`，5/5 通过；`cargo check --manifest-path src-tauri/Cargo.toml --locked` 通过。新 worktree 未构建前端，通过仅本次进程的 `TAURI_CONFIG.build.frontendDist` 引用已有本地 dist 完成类型/编译检查，不等同于安装包构建或已安装服务验证。
