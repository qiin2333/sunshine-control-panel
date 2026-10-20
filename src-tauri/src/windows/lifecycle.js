(function() {
    let _hidden = false;
    let _pauseStyleEl = null;

    Object.defineProperty(document, 'hidden', {
        get: function() { return _hidden; },
        configurable: true,
    });
    Object.defineProperty(document, 'visibilityState', {
        get: function() { return _hidden ? 'hidden' : 'visible'; },
        configurable: true,
    });

    function pauseAnimations() {
        if (!_pauseStyleEl) {
            _pauseStyleEl = document.createElement('style');
            _pauseStyleEl.id = '__gpu_pause_animations';
            _pauseStyleEl.textContent = '*, *::before, *::after { animation-play-state: paused !important; transition: none !important; }';
        }
        if (!_pauseStyleEl.parentNode) {
            (document.head || document.documentElement).appendChild(_pauseStyleEl);
        }
    }

    function resumeAnimations() {
        if (_pauseStyleEl && _pauseStyleEl.parentNode) {
            _pauseStyleEl.parentNode.removeChild(_pauseStyleEl);
        }
    }

    window.__setWebviewVisibility = function(visible) {
        var wasHidden = _hidden;
        _hidden = !visible;
        if (wasHidden !== _hidden) {
            console.log('[WebView Visibility] ' + (_hidden ? '进入休眠' : '恢复活跃'));
            if (_hidden) {
                pauseAnimations();
            } else {
                resumeAnimations();
            }
            document.dispatchEvent(new Event('visibilitychange'));
        }
    };

    // WebView 心跳：每 10 秒向 Rust 后端发送心跳信号
    // 若渲染进程崩溃，心跳停止，后端检测到后可触发自动恢复
    setInterval(function() {
        try {
            if (window.__TAURI_INTERNALS__) {
                window.__TAURI_INTERNALS__.invoke('webview_heartbeat', {generation: __LIFECYCLE_GENERATION__});
            }
        } catch(e) {}
    }, 10000);
    // 立即发送首次心跳
    try {
        if (window.__TAURI_INTERNALS__) {
            window.__TAURI_INTERNALS__.invoke('webview_heartbeat', {generation: __LIFECYCLE_GENERATION__});
        }
    } catch(e) {}
})();
