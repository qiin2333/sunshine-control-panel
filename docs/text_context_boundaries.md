# Windows 文本上下文边界

- 进程级服务在 `app.rs` 启动和退出，后台任务由 Tauri 应用状态持有并统一取消。关闭面板、最小化和切换焦点不停止 agent presence。
- 用户关闭文本上下文开关时立即停止观察并上报 `agent=false`；重新开启时按最近确认的串流状态恢复，退出后的迟到事件不能重新启动服务。
- 文本观察器仅在视频串流活动期间运行；暂停、结束或确认 Core 断开时停止。新 Core 用 `sessions-v1` 排除纯控制会话，旧 Core 保留 streaming 状态兼容。
- `text_context/agent.rs` 持有观察器和唯一的网络任务，先提交 capability，再发送同一代观察结果；`agent_id/generation` 隔离重启、迟到回调和旧观察结果。COM 调用不占用生命周期状态锁；停止时撤销代次并发出取消，不等待提供方返回。

- UIA 只在已有焦点、启用、非离屏及有效矩形检查通过，且控件是 Edit、Value.IsReadOnly 明确为 false 时报告 editable。
- TextEditPattern 可以存在于只读 Chrome Document，不作为可编辑证明；缺失/失败按未知处理，不读取输入内容。
- Document、富文本、contenteditable、自绘或无 ValuePattern 的编辑器可能漏报。这是主动收缩覆盖，可编辑判断不依赖应用名单或光标启发式。
- InputPane 通道仍保留，不能据此宣称旧客户端不再误弹。应配合 Moonlight V+ #595 的手动键盘策略。
- 合并源码不改变已安装程序，需正常集成和发布；主机缓存、焦点身份与 InputPane 各布局仍不具备全覆盖保证。

回归测试位于 `text_context.rs` 和 `text_context/agent.rs`，覆盖可编辑边界、传输顺序、失败重试及观察器换代。历史 Chrome 样本用于回归，不代表全应用和真实串流已完成验收。
