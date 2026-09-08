# Windows 文本上下文边界

- UIA 只在已有焦点、启用、非离屏及有效矩形检查通过，且控件是 Edit、Value.IsReadOnly 明确为 false 时报告 editable。
- TextEditPattern 可以存在于只读 Chrome Document，不作为可编辑证明；缺失/失败按未知处理，不读取输入内容。
- Document、富文本、contenteditable、自绘或无 ValuePattern 的编辑器可能漏报。这是主动收缩覆盖，不引入应用名单、光标启发式或协议字段。
- InputPane 通道仍保留，不能据此宣称旧客户端不再误弹。应配合 Moonlight V+ #595 的手动键盘策略。
- 合并源码不改变已安装程序，需正常集成和发布；主机缓存、焦点身份与 InputPane 各布局仍不具备全覆盖保证。

回归测试位于 `text_context.rs`，覆盖可写、只读、未知 Edit，以及 Document/其他控件。历史 Chrome 样本用于回归，不代表全应用和真实串流已完成验收。
