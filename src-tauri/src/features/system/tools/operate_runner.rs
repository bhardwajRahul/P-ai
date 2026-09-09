/// 取动作所在的脚本行号（用于失败回传定位）。
fn screenshot_content_hash(base64: &Option<String>) -> Option<u64> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    base64.as_ref().map(|text| {
        let mut hasher = DefaultHasher::new();
        text.hash(&mut hasher);
        hasher.finish()
    })
}

fn desktop_action_line(action: &DesktopScriptAction) -> usize {
    match action {
        DesktopScriptAction::MouseClick { line, .. }
        | DesktopScriptAction::MouseDrag { line, .. }
        | DesktopScriptAction::MouseMove { line, .. }
        | DesktopScriptAction::MouseButtonState { line, .. }
        | DesktopScriptAction::MouseScroll { line, .. }
        | DesktopScriptAction::Key { line, .. }
        | DesktopScriptAction::Text { line, .. }
        | DesktopScriptAction::Wait { line, .. }
        | DesktopScriptAction::WaitUntil { line, .. }
        | DesktopScriptAction::WindowList { line }
        | DesktopScriptAction::WindowActivate { line, .. }
        | DesktopScriptAction::Screenshot { line, .. }
        | DesktopScriptAction::Clipboard { line, .. }
        | DesktopScriptAction::App { line, .. } => *line,
    }
}

async fn run_operate_tool(
    input: OperateRequest,
    screenshots_root: &std::path::Path,
    include_base64: bool,
    blocked_apps: &[String],
) -> DesktopToolResult<OperateResponse> {
    let started = std::time::Instant::now();
    let mut warnings = Vec::<String>::new();
    if !ensure_dpi_awareness() {
        warnings.push("DPI 感知未生效（进程可能已被宿主设为其他感知模式）：高缩放或多显示器场景下坐标可能偏移，请用 screenshot 核对实际位置，或用 monitor= 指定显示器".to_string());
        runtime_log_warn("[桌面脚本] DPI 感知设置失败，高缩放或多显示器场景下坐标可能偏移".to_string());
    }
    let actions = parse_script(&input)?;
    // 单步重试预算（D4）：只对输入步骤的执行调用生效，钳制在 0~3，默认不重试
    let retry_budget = input.retry.unwrap_or(0).min(3);
    let total_actions = actions.len();
    runtime_log_info(format!(
        "[桌面脚本] 开始，任务=run_operate_tool，total_actions={}，timestamp={}",
        total_actions,
        now_iso()
    ));
    let mut enigo = enigo::Enigo::new(&enigo::Settings::default())
        .map_err(|err| DesktopToolError::internal_error(format!("创建 Enigo 失败：{err}")))?;
    let mut steps = Vec::<DesktopScriptStepResult>::new();
    let mut failure: Option<OperateFailure> = None;
    let mut latest_screenshot: Option<LatestScreenshotInfo> = None;
    let mut image_mime = None;
    let mut image_base64 = None;
    let mut width = None;
    let mut height = None;
    let mut latest_windows: Option<Vec<WindowInfo>> = None;
    // 执行前的前台（C4）：restore_focus=true 时结束后切回，恢复失败只记 warnings
    let initial_foreground = crate::platform::list_all_windows()
        .into_iter()
        .find(|w| w.focused)
        .map(|w| w.window_id);
    // 上次截图的内容哈希（F1）：画面相同时不重复传输 base64
    let mut last_screenshot_hash: Option<u64> = None;

    // 步骤失败即记录失败位置并停止后续步骤，已完成的步骤仍然返回（D1）
    macro_rules! run_step {
        ($line:expr, $call:expr) => {
            match $call.await {
                Ok(value) => value,
                Err(err) => {
                    failure = Some(OperateFailure { line: $line, message: err.message, focus_failed: None });
                    break;
                }
            }
        };
    }
    // 单步重试（D4）：输入步骤执行失败时按预算重试；参数非法不重试（写法错了重试也没用）。
    // 门控拦截/焦点策略/超时中断发生在执行调用之前，自然不在重试范围内。
    // 求值为 Some((结果, 重试次数))；预算耗尽时记录 failure 并求值为 None，调用方用 `None => break` 中断。
    macro_rules! run_step_retry {
        ($line:expr, $call:expr) => {{
            let mut __attempts: u32 = 0;
            loop {
                match $call.await {
                    Ok(__value) => break Some((__value, __attempts)),
                    Err(__err) => {
                        let __retryable = !matches!(__err.code, DesktopToolErrorCode::InvalidParams);
                        if __retryable && __attempts < retry_budget {
                            __attempts += 1;
                            runtime_log_warn(format!(
                                "[桌面脚本] 步骤失败准备重试，任务=run_operate_tool，line={}，attempt={}，原因={}",
                                $line, __attempts, __err.message
                            ));
                            sleep_duration(std::time::Duration::from_millis(200)).await;
                        } else {
                            failure = Some(OperateFailure { line: $line, message: __err.message, focus_failed: None });
                            break None;
                        }
                    }
                }
            }
        }};
    }
    // 声明了 target 的前台动作，执行前按 focus 策略处理目标窗口（C1/C2/C3），
    // 同时校验敏感应用门控（J1）：命中禁止名单即阻断
    macro_rules! ensure_foreground {
        ($line:expr, $target:expr, $focus:expr) => {
            match apply_foreground_policy(&$target, $focus, blocked_apps).await {
                Ok(note) => note,
                Err(block) => {
                    failure = Some(OperateFailure { line: $line, message: block.message, focus_failed: block.focus_failed });
                    break;
                }
            }
        };
    }
    let note_suffix = |note: Option<String>| note.map(|text| format!("，{text}")).unwrap_or_default();
    let retry_suffix = |attempts: u32| {
        if attempts > 0 {
            format!("，重试{attempts}次后成功")
        } else {
            String::new()
        }
    };

    for action in actions {
        // 脚本内超时中断（D3）：工具级超时是硬中断、已执行步骤会丢，
        // 这里在每步之间检查，超时也能回传已完成步骤与中断行号。
        if let Some(limit_ms) = input.timeout_ms {
            if started.elapsed().as_millis() > u128::from(limit_ms) {
                failure = Some(OperateFailure {
                    line: desktop_action_line(&action),
                    message: format!("脚本执行超过 timeout_ms={limit_ms}，已中断；已完成步骤见 steps，可从失败行继续"),
                    focus_failed: None,
                });
                break;
            }
        }
        match action {
            DesktopScriptAction::MouseClick { line, button, target, monitor, repeat, delay, pre_delay, press, window_target, focus, verify } => {
                let focus_note = ensure_foreground!(line, window_target, focus);
                let ((), retried) = match run_step_retry!(line, execute_mouse_click(&mut enigo, button, &target, monitor, repeat, delay, pre_delay, press)) {
                    Some(pair) => pair,
                    None => break,
                };
                let hit_note = mouse_hit_note(&target, monitor);
                let verify_note = if verify { post_action_verify_note() } else { String::new() };
                let step = DesktopScriptStepResult {
                    line,
                    kind: DesktopScriptStepKind::Mouse,
                    summary: format!("mouse click completed, repeat={repeat}{}{hit_note}{verify_note}{}", note_suffix(focus_note), retry_suffix(retried)),
                    ok: true,
                    saved_path: None,
                };
                runtime_log_info(format!(
                    "[桌面脚本] 步骤完成，任务=run_operate_tool，line={}，kind=MouseClick，summary={}",
                    line, step.summary
                ));
                steps.push(step);
            }
            DesktopScriptAction::MouseDrag { line, button, from, to, monitor, duration, pre_delay, window_target, focus } => {
                let focus_note = ensure_foreground!(line, window_target, focus);
                let ((), retried) = match run_step_retry!(line, execute_mouse_drag(&mut enigo, button, &from, &to, monitor, duration, pre_delay)) {
                    Some(pair) => pair,
                    None => break,
                };
                let step = DesktopScriptStepResult {
                    line,
                    kind: DesktopScriptStepKind::Mouse,
                    summary: format!("mouse drag completed{}{}", note_suffix(focus_note), retry_suffix(retried)),
                    ok: true,
                    saved_path: None,
                };
                runtime_log_info(format!(
                    "[桌面脚本] 步骤完成，任务=run_operate_tool，line={}，kind=MouseDrag，summary={}",
                    line, step.summary
                ));
                steps.push(step);
            }
            DesktopScriptAction::MouseMove { line, target, monitor, pre_delay, window_target, focus } => {
                let focus_note = ensure_foreground!(line, window_target, focus);
                let ((), retried) = match run_step_retry!(line, execute_mouse_move(&mut enigo, &target, monitor, pre_delay)) {
                    Some(pair) => pair,
                    None => break,
                };
                let step = DesktopScriptStepResult {
                    line,
                    kind: DesktopScriptStepKind::Mouse,
                    summary: format!("mouse move completed{}{}", note_suffix(focus_note), retry_suffix(retried)),
                    ok: true,
                    saved_path: None,
                };
                runtime_log_info(format!(
                    "[桌面脚本] 步骤完成，任务=run_operate_tool，line={}，kind=MouseMove，summary={}",
                    line, step.summary
                ));
                steps.push(step);
            }
            DesktopScriptAction::MouseButtonState { line, button, pressed, pre_delay, window_target, focus } => {
                let focus_note = ensure_foreground!(line, window_target, focus);
                let ((), retried) = match run_step_retry!(line, execute_mouse_button_state(&mut enigo, button, pressed, pre_delay)) {
                    Some(pair) => pair,
                    None => break,
                };
                let step = DesktopScriptStepResult {
                    line,
                    kind: DesktopScriptStepKind::Mouse,
                    summary: format!("mouse {} completed{}{}", if pressed { "down" } else { "up" }, note_suffix(focus_note), retry_suffix(retried)),
                    ok: true,
                    saved_path: None,
                };
                runtime_log_info(format!(
                    "[桌面脚本] 步骤完成，任务=run_operate_tool，line={}，kind=MouseButtonState，summary={}",
                    line, step.summary
                ));
                steps.push(step);
            }
            DesktopScriptAction::MouseScroll { line, horizontal, direction, repeat, delay, pre_delay, window_target, focus } => {
                let focus_note = ensure_foreground!(line, window_target, focus);
                let ((), retried) = match run_step_retry!(line, execute_mouse_scroll(&mut enigo, horizontal, direction, repeat, delay, pre_delay)) {
                    Some(pair) => pair,
                    None => break,
                };
                let axis = if horizontal { "horizontal" } else { "vertical" };
                let step = DesktopScriptStepResult {
                    line,
                    kind: DesktopScriptStepKind::Mouse,
                    summary: format!("mouse scroll completed, axis={axis}, repeat={repeat}{}{}", note_suffix(focus_note), retry_suffix(retried)),
                    ok: true,
                    saved_path: None,
                };
                runtime_log_info(format!(
                    "[桌面脚本] 步骤完成，任务=run_operate_tool，line={}，kind=MouseScroll，summary={}",
                    line, step.summary
                ));
                steps.push(step);
            }
            DesktopScriptAction::App { line, window, action, post_delay } => {
                // G2：名字选择器每次执行现查，不缓存句柄；解析失败（含候选）直接中断
                let window_id = match resolve_app_window(&window) {
                    Ok(id) => id,
                    Err(err) => {
                        failure = Some(OperateFailure { line, message: err.message, focus_failed: None });
                        break;
                    }
                };
                // 后台直连同样受门控约束（J1）：按解析出的句柄查标题，命中即阻断；
                // 窗口已不存在时不拦截，交由原有路径报错
                if !blocked_apps.is_empty() {
                    let hit = crate::platform::list_all_windows()
                        .into_iter()
                        .find(|w| w.window_id == window_id as usize)
                        .and_then(|w| blocked_app_hit(blocked_apps, &w.title).map(|keyword| (w.title, keyword.to_string())));
                    if let Some((title, keyword)) = hit {
                        failure = Some(OperateFailure { line, message: blocked_app_message(&title, &keyword), focus_failed: None });
                        break;
                    }
                }
                let ((verb, method, extra), retried) = match run_step_retry!(line, execute_app_action(window_id, action.clone(), post_delay)) {
                    Some(triple) => triple,
                    None => break,
                };
                let extra_text = extra.map(|e| format!(", {e}")).unwrap_or_default();
                let window_label = match &window {
                    AppWindowSelector::Id(id) => format!("window_id={id}"),
                    AppWindowSelector::Name(name) => format!("window=\"{name}\"（{window_id}）"),
                };
                let step = DesktopScriptStepResult {
                    line,
                    kind: DesktopScriptStepKind::App,
                    summary: format!("app {verb} completed, {window_label}, method={method}{extra_text}{}", retry_suffix(retried)),
                    ok: true,
                    saved_path: None,
                };
                runtime_log_info(format!(
                    "[桌面脚本] 步骤完成，任务=run_operate_tool，line={}，kind=App，summary={}",
                    line, step.summary
                ));
                steps.push(step);
            }
            DesktopScriptAction::Key { line, keys, repeat, delay, pre_delay, press, window_target, focus, verify } => {
                let focus_note = ensure_foreground!(line, window_target, focus);
                let ((), retried) = match run_step_retry!(line, execute_key_action(&mut enigo, &keys, line, repeat, delay, pre_delay, press)) {
                    Some(pair) => pair,
                    None => break,
                };
                let verify_note = if verify { post_action_verify_note() } else { String::new() };
                let step = DesktopScriptStepResult {
                    line,
                    kind: DesktopScriptStepKind::Key,
                    summary: format!("key action completed, combo={}, repeat={repeat}{}{verify_note}{}", keys.join("+"), note_suffix(focus_note), retry_suffix(retried)),
                    ok: true,
                    saved_path: None,
                };
                runtime_log_info(format!(
                    "[桌面脚本] 步骤完成，任务=run_operate_tool，line={}，kind=Key，summary={}",
                    line, step.summary
                ));
                steps.push(step);
            }
            DesktopScriptAction::Text { line, text, repeat, delay, pre_delay, window_target, focus, verify } => {
                let focus_note = ensure_foreground!(line, window_target, focus);
                let ((), retried) = match run_step_retry!(line, execute_text_action(&mut enigo, &text, repeat, delay, pre_delay)) {
                    Some(pair) => pair,
                    None => break,
                };
                let verify_note = if verify { post_action_verify_note() } else { String::new() };
                let step = DesktopScriptStepResult {
                    line,
                    kind: DesktopScriptStepKind::Text,
                    summary: format!("text input completed, chars={}, repeat={repeat}{}{verify_note}{}", text.chars().count(), note_suffix(focus_note), retry_suffix(retried)),
                    ok: true,
                    saved_path: None,
                };
                runtime_log_info(format!(
                    "[桌面脚本] 步骤完成，任务=run_operate_tool，line={}，kind=Text，summary={}",
                    line, step.summary
                ));
                steps.push(step);
            }
            DesktopScriptAction::Wait { line, duration } => {
                sleep_duration(duration).await;
                let step = DesktopScriptStepResult {
                    line,
                    kind: DesktopScriptStepKind::Wait,
                    summary: format!("wait completed, seconds={:.3}", duration.as_secs_f64()),
                    ok: true,
                    saved_path: None,
                };
                runtime_log_info(format!(
                    "[桌面脚本] 步骤完成，任务=run_operate_tool，line={}，kind=Wait，summary={}",
                    line, step.summary
                ));
                steps.push(step);
            }
            DesktopScriptAction::WaitUntil { line, condition, timeout } => {
                let description = run_step!(line, wait_until(&condition, timeout));
                let step = DesktopScriptStepResult {
                    line,
                    kind: DesktopScriptStepKind::Wait,
                    summary: format!("wait until completed, {description}"),
                    ok: true,
                    saved_path: None,
                };
                runtime_log_info(format!(
                    "[桌面脚本] 步骤完成，任务=run_operate_tool，line={}，kind=WaitUntil，summary={}",
                    line, step.summary
                ));
                steps.push(step);
            }
            DesktopScriptAction::WindowList { line } => {
                let windows = crate::platform::list_all_windows();
                // 枚举为空且平台有已知限制时把原因讲清楚，避免模型反复重试（H2）
                let summary = if windows.is_empty() {
                    match crate::platform::platform_capability_hint() {
                        Some(hint) => format!("window list completed, count=0（{hint}）"),
                        None => "window list completed, count=0".to_string(),
                    }
                } else {
                    format!("window list completed, count={}", windows.len())
                };
                let step = DesktopScriptStepResult {
                    line,
                    kind: DesktopScriptStepKind::Window,
                    summary,
                    ok: true,
                    saved_path: None,
                };
                runtime_log_info(format!(
                    "[桌面脚本] 步骤完成，任务=run_operate_tool，line={}，kind=WindowList，summary={}",
                    line, step.summary
                ));
                latest_windows = Some(windows);
                steps.push(step);
            }
            DesktopScriptAction::WindowActivate { line, target } => {
                let window = match resolve_foreground_window(&target) {
                    Ok(window) => window,
                    Err(err) => {
                        failure = Some(OperateFailure { line, message: err.message, focus_failed: None });
                        break;
                    }
                };
                // 激活同样受门控约束（J1）：目标命中禁止名单即阻断，不切前台
                if let Some(keyword) = blocked_app_hit(blocked_apps, &window.title) {
                    failure = Some(OperateFailure { line, message: blocked_app_message(&window.title, keyword), focus_failed: None });
                    break;
                }
                let before = foreground_title(&crate::platform::list_all_windows());
                let window_id = window.window_id;
                let activated = match tokio::task::spawn_blocking(move || crate::platform::activate_window(window_id)).await {
                    Ok((_, activated)) => activated,
                    Err(err) => {
                        failure = Some(OperateFailure { line, message: format!("激活窗口任务失败：{err}"), focus_failed: None });
                        break;
                    }
                };
                if !activated {
                    let info = build_focus_failure(&window, before);
                    let message = format!(
                        "激活窗口失败：「{}」（{}），窗口存在={}，可见={}，最小化={}，前台「{}」->「{}」；建议 {:?}",
                        info.target_title, info.target_window_id, info.alive, info.visible, info.minimized,
                        info.foreground_before, info.foreground_after, info.suggested_recovery
                    );
                    failure = Some(OperateFailure { line, message, focus_failed: Some(info) });
                    break;
                }
                let step = DesktopScriptStepResult {
                    line,
                    kind: DesktopScriptStepKind::Window,
                    summary: format!("window activate completed, window_id={}, title={}", window.window_id, window.title),
                    ok: true,
                    saved_path: None,
                };
                runtime_log_info(format!(
                    "[桌面脚本] 步骤完成，任务=run_operate_tool，line={}，kind=WindowActivate，summary={}",
                    line, step.summary
                ));
                steps.push(step);
            }
            DesktopScriptAction::Screenshot { line, mode, save_path, quality, elements, include_text, max_pixels } => {
                let (result, mode_name, ui_tree, pruned) =
                    run_step!(line, execute_screenshot_action(&mode, save_path, quality, screenshots_root, include_base64, elements, include_text, max_pixels));
                // J3：默认路径截图顺手裁剪过期文件，只在清掉东西时告诉模型
                if pruned > 0 {
                    warnings.push(format!("已清理 {pruned} 个过期临时截图（仅清理 operate_*.webp 默认路径，保留最新 200 个且 7 天内）"));
                }
                let tree_summary = match &ui_tree {
                    // E4/H3：空树时把平台原因讲清楚，避免模型反复重试；Windows 沿用原有文案
                    Some(elems) if elems.is_empty() => {
                        match crate::platform::ui_tree_empty_hint() {
                            Some(hint) => format!("，{hint}"),
                            None => "，控件树为空或目标窗口未暴露 UIA".to_string(),
                        }
                    }
                    Some(elems) if elems.len() >= crate::platform::MAX_ELEMENTS => format!(
                        "，控件树元素数={}（已达上限，可能被截断；请用 window_id 或 region 缩小范围）",
                        elems.len()
                    ),
                    Some(elems) => format!("，控件树元素数={}", elems.len()),
                    None => String::new(),
                };
                // F1：画面与本次调用的上次截图相同时不重复传输 base64，模型沿用上下文中的上一张图
                let hash = screenshot_content_hash(&result.image_base64);
                let (dedup, base64_out) = match (hash, last_screenshot_hash) {
                    (Some(current), Some(previous)) if current == previous => (true, None),
                    _ => (false, result.image_base64.clone()),
                };
                last_screenshot_hash = hash;
                let dedup_note = if dedup {
                    format!("，dedup=true（画面与上次截图相同，未重复传输，hash={}）", hash.unwrap_or(0))
                } else {
                    String::new()
                };
                latest_screenshot = Some(LatestScreenshotInfo {
                    mode: mode_name.clone(),
                    width: result.width,
                    height: result.height,
                    saved_path: result.path.clone(),
                    tree: ui_tree,
                });
                image_mime = Some(result.image_mime.clone());
                image_base64 = base64_out;
                width = Some(result.width);
                height = Some(result.height);
                let step = DesktopScriptStepResult {
                    line,
                    kind: DesktopScriptStepKind::Screenshot,
                    summary: format!("screenshot completed, mode={mode_name}{tree_summary}{dedup_note}"),
                    ok: true,
                    saved_path: result.path,
                };
                runtime_log_info(format!(
                    "[桌面脚本] 步骤完成，任务=run_operate_tool，line={}，kind=Screenshot，summary={}",
                    line, step.summary
                ));
                steps.push(step);
            }
            DesktopScriptAction::Clipboard { line, op } => {
                // A4：剪贴板当数据通道用；read 只读不改，write 显式覆盖（意图明确才用）
                let summary = match run_step_retry!(line, async {
                    match &op {
                        ClipboardOp::Read => execute_clipboard_read(),
                        ClipboardOp::Write(text) => execute_clipboard_write(text),
                    }
                }) {
                    Some((text, _)) => text,
                    None => break,
                };
                let step = DesktopScriptStepResult {
                    line,
                    kind: DesktopScriptStepKind::Clipboard,
                    summary,
                    ok: true,
                    saved_path: None,
                };
                runtime_log_info(format!(
                    "[桌面脚本] 步骤完成，任务=run_operate_tool，line={}，kind=Clipboard，summary={}",
                    line, step.summary
                ));
                steps.push(step);
            }
        }
    }

    // 执行后焦点恢复（C4）：restore_focus=true 时把前台切回执行前的窗口，失败只记 warnings，不影响主结果
    if input.restore_focus.unwrap_or(false) {
        if let Some(window_id) = initial_foreground {
            let restored = tokio::task::spawn_blocking(move || crate::platform::activate_window(window_id))
                .await
                .map(|(_, ok)| ok)
                .unwrap_or(false);
            if restored {
                runtime_log_info("[桌面脚本] 已按 restore_focus 恢复执行前的前台窗口".to_string());
            } else {
                warnings.push(format!("restore_focus 恢复前台失败（window_id={window_id}，窗口可能已关闭）；不影响本次执行结果"));
                runtime_log_warn(format!("[桌面脚本] restore_focus 恢复前台失败，window_id={window_id}"));
            }
        } else {
            warnings.push("restore_focus 未生效：执行前没有记录到前台窗口；不影响本次执行结果".to_string());
        }
    }

    let elapsed_ms = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
    let ok = failure.is_none();
    runtime_log_info(format!(
        "[桌面脚本] {}，任务=run_operate_tool，executed_count={}，failed_line={}，elapsed_ms={}，latest_screenshot={}，image_mime={}，has_image_base64={}，width={}，height={}",
        if ok { "完成" } else { "中断" },
        steps.len(),
        failure.as_ref().map(|item| item.line.to_string()).unwrap_or_else(|| "-".to_string()),
        elapsed_ms,
        latest_screenshot
            .as_ref()
            .map(|shot| format!(
                "mode={},width={},height={},saved_path={}",
                shot.mode,
                shot.width,
                shot.height,
                shot.saved_path.as_deref().unwrap_or("-")
            ))
            .unwrap_or_else(|| "none".to_string()),
        image_mime.as_deref().unwrap_or("-"),
        image_base64.as_ref().map(|_| true).unwrap_or(false),
        width
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string()),
        height
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string())
    ));

    // 操作审计（J2）：逐步骤记录时间、动作、目标、结果，关键操作可追溯
    let audit_time = now_iso();
    for step in &steps {
        runtime_log_info(format!(
            "[桌面脚本][审计] 时间={audit_time}，line={}，kind={:?}，结果=成功，摘要={}",
            step.line, step.kind, step.summary
        ));
    }
    if let Some(item) = &failure {
        runtime_log_info(format!(
            "[桌面脚本][审计] 时间={audit_time}，line={}，结果=失败，原因={}",
            item.line, item.message
        ));
    }

    Ok(OperateResponse {
        ok,
        executed_count: steps.len(),
        elapsed_ms,
        steps,
        failure,
        latest_screenshot,
        image_mime,
        image_base64,
        width,
        height,
        windows: latest_windows,
        warnings: if warnings.is_empty() { None } else { Some(warnings) },
    })
}

/// 清空指定会话的 operate 截图临时目录（temp/screenshots/{conversation_id}/），
/// 会话压缩/归档/删除/撤回时调用。返回 (删除文件数, 删除子目录数)；目录不存在时视为已清空。
fn clear_operate_screenshots_temp(
    data_path: &PathBuf,
    conversation_id: &str,
) -> Result<(usize, usize), String> {
    let dir = app_root_from_data_path(data_path)
        .join("temp")
        .join("screenshots")
        .join(conversation_id);
    let mut removed_files = 0usize;
    let mut removed_dirs = 0usize;
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                std::fs::remove_file(&path).map_err(|err| {
                    format!("清理 operate 截图失败（{}）：{err}", path.to_string_lossy())
                })?;
                removed_files = removed_files.saturating_add(1);
            } else if path.is_dir() {
                std::fs::remove_dir_all(&path).map_err(|err| {
                    format!("清理 operate 截图子目录失败（{}）：{err}", path.to_string_lossy())
                })?;
                removed_dirs = removed_dirs.saturating_add(1);
            }
        }
    }
    Ok((removed_files, removed_dirs))
}

#[cfg(test)]
mod operate_tool_tests {
    use super::*;

    fn parse_single(script: &str) -> DesktopScriptAction {
        parse_script(&OperateRequest { script: script.to_string(), timeout_ms: None, retry: None, restore_focus: None })
            .unwrap()
            .into_iter()
            .next()
            .unwrap()
    }

    #[test]
    fn parse_mouse_click_script() {
        match parse_single("mouse left click @0.50,0.10 repeat=2 delay=0.1") {
            DesktopScriptAction::MouseClick { repeat, .. } => assert_eq!(repeat, 2),
            _ => panic!("expected mouse click"),
        }
    }

    #[test]
    fn parse_mouse_move_script() {
        match parse_single("mouse move @0.25,0.40 pre_delay=0.2") {
            DesktopScriptAction::MouseMove { target, pre_delay, .. } => {
                assert!((target.x - 0.25).abs() < f64::EPSILON);
                assert!((target.y - 0.40).abs() < f64::EPSILON);
                assert_eq!(pre_delay, std::time::Duration::from_millis(200));
            }
            _ => panic!("expected mouse move"),
        }
    }

    #[test]
    fn parse_mouse_drag_script() {
        match parse_single("mouse left drag @0.10,0.20 @0.80,0.90") {
            DesktopScriptAction::MouseDrag { button, from, to, duration, .. } => {
                assert_eq!(button, OperateMouseButton::Left);
                assert!((from.x - 0.10).abs() < f64::EPSILON);
                assert!((from.y - 0.20).abs() < f64::EPSILON);
                assert!((to.x - 0.80).abs() < f64::EPSILON);
                assert!((to.y - 0.90).abs() < f64::EPSILON);
                assert!(duration.is_none());
            }
            _ => panic!("expected mouse drag"),
        }
    }

    #[test]
    fn parse_mouse_drag_with_duration() {
        match parse_single("mouse right drag @0.0,0.0 @1.0,1.0 duration=0.4 pre_delay=0.1") {
            DesktopScriptAction::MouseDrag { button, duration, pre_delay, .. } => {
                assert_eq!(button, OperateMouseButton::Right);
                assert_eq!(duration, Some(std::time::Duration::from_secs_f64(0.4)));
                assert_eq!(pre_delay, std::time::Duration::from_millis(100));
            }
            _ => panic!("expected mouse drag"),
        }
    }

    #[test]
    fn parse_mouse_drag_requires_two_points() {
        let err = parse_script(&OperateRequest { script: "mouse left drag @0.1,0.1".to_string(), timeout_ms: None, retry: None, restore_focus: None }).unwrap_err();
        assert!(err.message.contains("拖拽格式"));
    }

    #[test]
    fn parse_foreground_target_by_window_id() {
        match parse_single("mouse left click @0.5,0.5 target=12345 focus=strict") {
            DesktopScriptAction::MouseClick { window_target, focus, .. } => {
                assert!(matches!(window_target, Some(ForegroundTarget::WindowId(12345))));
                assert_eq!(focus, FocusPolicy::Strict);
            }
            _ => panic!("expected mouse click"),
        }
    }

    #[test]
    fn parse_foreground_target_by_hex_window_id() {
        match parse_single("key Enter target=0x1A2B") {
            DesktopScriptAction::Key { window_target, .. } => {
                assert!(matches!(window_target, Some(ForegroundTarget::WindowId(0x1A2B))));
            }
            _ => panic!("expected key"),
        }
    }

    #[test]
    fn parse_foreground_target_by_title_defaults_to_verify() {
        match parse_single("text \"hello\" target=\"记事本\"") {
            DesktopScriptAction::Text { window_target, focus, .. } => {
                match window_target {
                    Some(ForegroundTarget::Title(title)) => assert_eq!(title, "记事本"),
                    other => panic!("expected title target, got {other:?}"),
                }
                assert_eq!(focus, FocusPolicy::Verify);
            }
            _ => panic!("expected text"),
        }
    }

    #[test]
    fn parse_mouse_scroll_accepts_target_and_focus() {
        match parse_single("mouse scroll_down repeat=3 target=\"浏览器\" focus=best_effort") {
            DesktopScriptAction::MouseScroll { repeat, window_target, focus, .. } => {
                assert_eq!(repeat, 3);
                assert!(matches!(window_target, Some(ForegroundTarget::Title(_))));
                assert_eq!(focus, FocusPolicy::BestEffort);
            }
            _ => panic!("expected mouse scroll"),
        }
    }

    #[test]
    fn parse_mouse_drag_accepts_target() {
        match parse_single("mouse left drag @0.1,0.1 @0.9,0.9 target=42") {
            DesktopScriptAction::MouseDrag { window_target, focus, .. } => {
                assert!(matches!(window_target, Some(ForegroundTarget::WindowId(42))));
                assert_eq!(focus, FocusPolicy::Verify);
            }
            _ => panic!("expected mouse drag"),
        }
    }

    #[test]
    fn parse_focus_without_target_is_rejected() {
        let err = parse_script(&OperateRequest { script: "key Enter focus=strict".to_string(), timeout_ms: None, retry: None, restore_focus: None }).unwrap_err();
        assert!(err.message.contains("focus 必须与 target"));
    }

    #[test]
    fn parse_focus_invalid_value_is_rejected() {
        let err = parse_script(&OperateRequest { script: "text \"hi\" target=\"x\" focus=always".to_string(), timeout_ms: None, retry: None, restore_focus: None }).unwrap_err();
        assert!(err.message.contains("focus 非法"));
    }

    #[test]
    fn parse_target_invalid_value_is_rejected() {
        let err = parse_script(&OperateRequest { script: "text \"hi\" target=notepad".to_string(), timeout_ms: None, retry: None, restore_focus: None }).unwrap_err();
        assert!(err.message.contains("windowId 非法"));
    }

    #[test]
    fn operate_failure_omits_focus_failed_when_absent() {
        let failure = OperateFailure { line: 3, message: "boom".to_string(), focus_failed: None };
        let value = serde_json::to_value(&failure).unwrap();
        assert_eq!(value["line"], 3);
        assert_eq!(value["message"], "boom");
        assert!(value.get("focusFailed").is_none());
    }

    #[test]
    fn focus_failure_serializes_camel_case_fields() {
        let info = FocusFailureInfo {
            target_window_id: 42,
            target_title: "记事本".to_string(),
            alive: true,
            visible: false,
            minimized: false,
            foreground_before: "A".to_string(),
            foreground_after: "B".to_string(),
            suggested_recovery: vec!["unhide_app".to_string()],
        };
        let value = serde_json::to_value(&info).unwrap();
        assert_eq!(value["targetWindowId"], 42);
        assert_eq!(value["foregroundBefore"], "A");
        assert_eq!(value["suggestedRecovery"][0], "unhide_app");
    }

    #[test]
    fn parse_mouse_move_requires_point() {
        let err = parse_script(&OperateRequest { script: "mouse move".to_string(), timeout_ms: None, retry: None, restore_focus: None }).unwrap_err();
        assert!(err.message.contains("第 1 行 mouse"));
        assert!(err.message.contains("移动格式"));
    }

    #[test]
    fn parse_app_click_by_element_ref() {
        match parse_single("app 0x1a2b click el=3 pre_delay=0.1") {
            DesktopScriptAction::App { window, action, .. } => {
                assert!(matches!(window, AppWindowSelector::Id(0x1a2b)));
                match action {
                    AppScriptAction::Click { target, repeat, .. } => {
                        assert!(matches!(target, AppScriptTarget::Element(3)));
                        assert_eq!(repeat, 1);
                    }
                    _ => panic!("expected click action"),
                }
            }
            _ => panic!("expected app action"),
        }
    }

    #[test]
    fn parse_app_click_by_point() {
        match parse_single("app 123 click @0.50,0.50 repeat=2") {
            DesktopScriptAction::App { window, action, .. } => {
                assert!(matches!(window, AppWindowSelector::Id(123)));
                match action {
                    AppScriptAction::Click { target, repeat, .. } => {
                        assert!(matches!(target, AppScriptTarget::Point(_)));
                        assert_eq!(repeat, 2);
                    }
                    _ => panic!("expected click action"),
                }
            }
            _ => panic!("expected app action"),
        }
    }

    #[test]
    fn parse_app_setvalue_rejects_point_target() {
        let err = parse_script(&OperateRequest { script: "app 1 setvalue @0.5,0.5 \"hi\"".to_string(), timeout_ms: None, retry: None, restore_focus: None }).unwrap_err();
        assert!(err.message.contains("setvalue"));
        assert!(err.message.contains("el="));
    }

    #[test]
    fn parse_app_scroll_script() {
        match parse_single("app 45 scroll_down el=7 repeat=3 delay=0.2") {
            DesktopScriptAction::App { window, action, .. } => {
                assert!(matches!(window, AppWindowSelector::Id(45)));
                match action {
                    AppScriptAction::Scroll { target, horizontal, positive, repeat, delay, .. } => {
                        assert!(matches!(target, AppScriptTarget::Element(7)));
                        assert!(!horizontal);
                        assert!(positive);
                        assert_eq!(repeat, 3);
                        assert_eq!(delay, std::time::Duration::from_millis(200));
                    }
                    _ => panic!("expected scroll action"),
                }
            }
            _ => panic!("expected app action"),
        }
    }

    #[test]
    fn parse_app_scroll_left_is_horizontal() {
        match parse_single("app 45 scroll_left el=7") {
            DesktopScriptAction::App { action, .. } => {
                assert!(matches!(
                    action,
                    AppScriptAction::Scroll { horizontal: true, positive: false, .. }
                ));
            }
            _ => panic!("expected app action"),
        }
    }

    #[test]
    fn parse_app_window_by_name() {
        match parse_single("app \"记事本\" click el=1") {
            DesktopScriptAction::App { window, action, .. } => {
                assert!(matches!(window, AppWindowSelector::Name(_)));
                assert!(matches!(action, AppScriptAction::Click { .. }));
            }
            _ => panic!("expected app action"),
        }
    }

    #[test]
    fn parse_app_key_script() {
        match parse_single("app 663002 key Enter") {
            DesktopScriptAction::App { window, action, .. } => {
                assert!(matches!(window, AppWindowSelector::Id(663002)));
                match action {
                    AppScriptAction::Key { keys, repeat, .. } => {
                        assert_eq!(keys, vec!["Enter".to_string()]);
                        assert_eq!(repeat, 1);
                    }
                    _ => panic!("expected key action"),
                }
            }
            _ => panic!("expected app action"),
        }
    }

    #[test]
    fn parse_app_key_combo_script() {
        match parse_single("app 10 key Control+A repeat=2 delay=0.1") {
            DesktopScriptAction::App { action: AppScriptAction::Key { keys, repeat, delay, .. }, .. } => {
                assert_eq!(keys, vec!["Control".to_string(), "A".to_string()]);
                assert_eq!(repeat, 2);
                assert_eq!(delay, std::time::Duration::from_millis(100));
            }
            _ => panic!("expected app key action"),
        }
    }

    #[test]
    fn parse_app_getvalue_script() {
        match parse_single("app 77 getvalue el=5") {
            DesktopScriptAction::App { window, action: AppScriptAction::GetValue { el }, .. } => {
                assert!(matches!(window, AppWindowSelector::Id(77)));
                assert_eq!(el, 5);
            }
            _ => panic!("expected app getvalue action"),
        }
    }

    #[test]
    fn parse_app_getvalue_rejects_point_target() {
        let err = parse_script(&OperateRequest { script: "app 1 getvalue @0.5,0.5".to_string(), timeout_ms: None, retry: None, restore_focus: None }).unwrap_err();
        assert!(err.message.contains("getvalue"));
        assert!(err.message.contains("el="));
    }

    #[test]
    fn parse_app_post_delay_script() {
        match parse_single("app 9 setvalue el=2 \"abc\" post_delay=1.5") {
            DesktopScriptAction::App { post_delay, .. } => {
                assert_eq!(post_delay, std::time::Duration::from_millis(1500));
            }
            _ => panic!("expected app action"),
        }
    }

    #[test]
    fn parse_app_click_dblclick_script() {
        match parse_single("app 8 click el=4 dblclick=true") {
            DesktopScriptAction::App { action: AppScriptAction::Click { target, dblclick, .. }, .. } => {
                assert!(matches!(target, AppScriptTarget::Element(4)));
                assert!(dblclick);
            }
            _ => panic!("expected app click action"),
        }
    }

    #[test]
    fn parse_app_dblclick_rejects_bad_value() {
        let err = parse_script(&OperateRequest { script: "app 8 click @0.5,0.5 dblclick=yes".to_string(), timeout_ms: None, retry: None, restore_focus: None }).unwrap_err();
        assert!(err.message.contains("dblclick") || err.message.contains("布尔参数非法"));
    }

    #[test]
    fn parse_screenshot_window_id_script() {
        match parse_single("screenshot window_id=0x1f elements=true") {
            DesktopScriptAction::Screenshot { mode, elements, .. } => {
                assert!(matches!(mode, ScreenshotModeSpec::WindowId(0x1f)));
                assert!(elements);
            }
            _ => panic!("expected screenshot action"),
        }
    }

    #[test]
    fn parse_screenshot_window_id_conflicts_with_region() {
        let err = parse_script(&OperateRequest { script: "screenshot window_id=1 region=@0.1,0.1,0.2,0.2".to_string(), timeout_ms: None, retry: None, restore_focus: None }).unwrap_err();
        assert!(err.message.contains("window_id"));
    }

    #[test]
    fn parse_key_script() {
        match parse_single("key Control+L") {
            DesktopScriptAction::Key { keys, .. } => assert_eq!(keys, vec!["Control".to_string(), "L".to_string()]),
            _ => panic!("expected key action"),
        }
    }

    #[test]
    fn parse_text_requires_quotes() {
        let err = parse_script(&OperateRequest { script: "text hello".to_string(), timeout_ms: None, retry: None, restore_focus: None }).unwrap_err();
        assert!(err.message.contains("第 1 行 text"));
        assert!(err.message.contains("双引号"));
    }

    #[test]
    fn parse_text_escape_newline_decodes_to_real_newline() {
        match parse_single(r#"text "第一行\n第二行""#) {
            DesktopScriptAction::Text { text, .. } => assert_eq!(text, "第一行\n第二行"),
            _ => panic!("expected text action"),
        }
    }

    #[test]
    fn parse_text_multiline_inside_quotes_stays_single_action() {
        let script = "text \"第一行\n第二行\"";
        let actions = parse_script(&OperateRequest { script: script.to_string(), timeout_ms: None, retry: None, restore_focus: None }).unwrap();
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            DesktopScriptAction::Text { text, .. } => assert_eq!(text, "第一行\n第二行"),
            _ => panic!("expected text action"),
        }
    }

    #[test]
    fn parse_script_newline_outside_quotes_splits_actions() {
        let script = "text \"第一行\"\ntext \"第二行\"\nscreenshot";
        let actions = parse_script(&OperateRequest { script: script.to_string(), timeout_ms: None, retry: None, restore_focus: None }).unwrap();
        assert_eq!(actions.len(), 3);
    }

    #[test]
    fn parse_script_multiline_line_numbers_are_accurate() {
        let script = "text \"a\"\nkey Enter\nscreenshot";
        let actions = parse_script(&OperateRequest { script: script.to_string(), timeout_ms: None, retry: None, restore_focus: None }).unwrap();
        match &actions[1] {
            DesktopScriptAction::Key { line, .. } => assert_eq!(*line, 2),
            other => panic!("expected key action at line 2, got {other:?}"),
        }
    }

    #[test]
    fn parse_script_unclosed_quote_reports_line_number() {
        let script = "text \"第一行\n第二行";
        let err = parse_script(&OperateRequest { script: script.to_string(), timeout_ms: None, retry: None, restore_focus: None }).unwrap_err();
        assert!(err.message.contains("第 1 行"));
        assert!(err.message.contains("双引号未闭合"));
    }

    #[test]
    fn screenshot_save_requires_absolute_path() {
        let err = parse_script(&OperateRequest { script: r#"screenshot save="tmp/shot.webp""#.to_string(), timeout_ms: None, retry: None, restore_focus: None }).unwrap_err();
        assert!(err.message.contains("第 1 行 screenshot"));
        assert!(err.message.contains("绝对路径"));
    }

    #[test]
    fn mouse_coordinates_must_be_normalized() {
        let err = parse_script(&OperateRequest { script: "mouse left click @1.2,0.5".to_string(), timeout_ms: None, retry: None, restore_focus: None }).unwrap_err();
        assert!(err.message.contains("第 1 行 mouse"));
        assert!(err.message.contains("0.0~1.0"));
    }

    #[test]
    fn screenshot_region_should_parse() {
        match parse_single("screenshot region=@0.10,0.10,0.80,0.60") {
            DesktopScriptAction::Screenshot { mode: ScreenshotModeSpec::Region(_), .. } => {}
            _ => panic!("expected screenshot region"),
        }
    }

    #[test]
    fn screenshot_tree_true_should_parse() {
        match parse_single("screenshot elements=true") {
            DesktopScriptAction::Screenshot { elements, .. } => assert!(elements),
            _ => panic!("expected screenshot action"),
        }
    }

    #[test]
    fn screenshot_tree_default_should_be_false() {
        match parse_single("screenshot") {
            DesktopScriptAction::Screenshot { elements, .. } => assert!(!elements),
            _ => panic!("expected screenshot action"),
        }
    }

    #[test]
    fn screenshot_tree_invalid_should_reject() {
        let err = parse_script(&OperateRequest { script: "screenshot elements=yes".to_string(), timeout_ms: None, retry: None, restore_focus: None }).unwrap_err();
        assert!(err.message.contains("第 1 行 screenshot"));
        assert!(err.message.contains("elements 非法"));
    }

    #[test]
    fn clear_operate_screenshots_temp_should_only_remove_screenshots() {
        let root = std::env::temp_dir().join("easy-call-ai-clear-operate-test");
        let _ = std::fs::remove_dir_all(&root);
        let data_path = root.join("config");
        let conversation_id = "convo-test-1";
        let screenshots = root
            .join("temp")
            .join("screenshots")
            .join(conversation_id);
        let sub = screenshots.join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(screenshots.join("a.webp"), b"a").unwrap();
        std::fs::write(sub.join("b.webp"), b"b").unwrap();
        // 其他会话的截图不应被清理
        let other = root.join("temp").join("screenshots").join("convo-other");
        std::fs::create_dir_all(&other).unwrap();
        std::fs::write(other.join("keep.webp"), b"keep").unwrap();
        let records = root.join("temp").join("apply_patch").join("records");
        let blobs = root.join("temp").join("apply_patch").join("blobs");
        std::fs::create_dir_all(&records).unwrap();
        std::fs::create_dir_all(&blobs).unwrap();
        std::fs::write(records.join("r.json"), b"{}").unwrap();
        std::fs::write(blobs.join("b.json"), b"{}").unwrap();

        let (files, dirs) = clear_operate_screenshots_temp(&data_path, conversation_id).unwrap();
        assert_eq!(files, 1, "top-level screenshot file should be removed");
        assert_eq!(dirs, 1, "nested screenshot dir should be removed recursively");
        assert!(!screenshots.join("a.webp").exists());
        assert!(!sub.exists(), "nested dir with inner file should be gone");
        assert!(
            !screenshots.join("sub").join("b.webp").exists(),
            "inner screenshot file should be gone with its dir"
        );
        assert!(
            other.join("keep.webp").exists(),
            "other conversation screenshots must survive"
        );
        assert!(
            records.join("r.json").exists(),
            "apply_patch records must survive cleanup"
        );
        assert!(
            blobs.join("b.json").exists(),
            "apply_patch blobs must survive cleanup"
        );

        let _ = std::fs::remove_dir_all(&root);
    }
}
