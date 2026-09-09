use enigo::{Keyboard, Mouse};

use crate::platform::AppTarget;

/// 设置进程 DPI 感知（per-monitor v2）。成功前每次调用都会重试：
/// 首次调用可能因为宿主窗口尚未就绪而失败，后续调用仍有机会成功；
/// 一旦成功就不再重复调用，避免每次都走一遍系统接口。
/// 返回 true 表示坐标映射已按物理像素对齐；false 表示感知未生效，
/// 高缩放或多显示器场景下坐标可能有偏移（B3）。
fn ensure_dpi_awareness() -> bool {
    static OK: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if OK.load(std::sync::atomic::Ordering::Relaxed) {
        return true;
    }
    #[cfg(target_os = "windows")]
    {
        if enigo::set_dpi_awareness().is_ok() {
            OK.store(true, std::sync::atomic::Ordering::Relaxed);
            return true;
        }
        false
    }
    #[cfg(not(target_os = "windows"))]
    {
        OK.store(true, std::sync::atomic::Ordering::Relaxed);
        true
    }
}

fn map_mouse_button(button: OperateMouseButton) -> enigo::Button {
    match button {
        OperateMouseButton::Left => enigo::Button::Left,
        OperateMouseButton::Right => enigo::Button::Right,
        OperateMouseButton::Middle => enigo::Button::Middle,
        OperateMouseButton::Back => enigo::Button::Back,
        OperateMouseButton::Forward => enigo::Button::Forward,
    }
}

fn map_input_err(err: enigo::InputError, context: &str) -> DesktopToolError {
    DesktopToolError::internal_error(format!("{context}: {err}"))
}

fn parse_named_key(name: &str) -> Option<enigo::Key> {
    let normalized = name.trim().to_lowercase().replace(['_', ' ', '-'], "");
    match normalized.as_str() {
        "ctrl" | "control" => Some(enigo::Key::Control),
        "lctrl" | "leftcontrol" => Some(enigo::Key::LControl),
        "rctrl" | "rightcontrol" => Some(enigo::Key::RControl),
        "shift" => Some(enigo::Key::Shift),
        "lshift" | "leftshift" => Some(enigo::Key::LShift),
        "rshift" | "rightshift" => Some(enigo::Key::RShift),
        "alt" | "option" => Some(enigo::Key::Alt),
        "meta" | "win" | "windows" | "command" | "cmd" => Some(enigo::Key::Meta),
        "enter" | "return" => Some(enigo::Key::Return),
        "tab" => Some(enigo::Key::Tab),
        "esc" | "escape" => Some(enigo::Key::Escape),
        "space" | "spacebar" => Some(enigo::Key::Space),
        "backspace" => Some(enigo::Key::Backspace),
        "delete" | "del" => Some(enigo::Key::Delete),
        "insert" => {
            #[cfg(any(target_os = "windows", target_os = "linux"))]
            { Some(enigo::Key::Insert) }
            #[cfg(target_os = "macos")]
            { None }
        }
        "up" | "arrowup" => Some(enigo::Key::UpArrow),
        "down" | "arrowdown" => Some(enigo::Key::DownArrow),
        "left" | "arrowleft" => Some(enigo::Key::LeftArrow),
        "right" | "arrowright" => Some(enigo::Key::RightArrow),
        "home" => Some(enigo::Key::Home),
        "end" => Some(enigo::Key::End),
        "pageup" => Some(enigo::Key::PageUp),
        "pagedown" => Some(enigo::Key::PageDown),
        "capslock" => Some(enigo::Key::CapsLock),
        "printscreen" => {
            #[cfg(any(target_os = "windows", target_os = "linux"))]
            { Some(enigo::Key::PrintScr) }
            #[cfg(target_os = "macos")]
            { None }
        }
        "pause" => {
            #[cfg(any(target_os = "windows", target_os = "linux"))]
            { Some(enigo::Key::Pause) }
            #[cfg(target_os = "macos")]
            { None }
        }
        "numlock" => {
            #[cfg(any(target_os = "windows", target_os = "linux"))]
            { Some(enigo::Key::Numlock) }
            #[cfg(target_os = "macos")]
            { None }
        }
        "f1" => Some(enigo::Key::F1),
        "f2" => Some(enigo::Key::F2),
        "f3" => Some(enigo::Key::F3),
        "f4" => Some(enigo::Key::F4),
        "f5" => Some(enigo::Key::F5),
        "f6" => Some(enigo::Key::F6),
        "f7" => Some(enigo::Key::F7),
        "f8" => Some(enigo::Key::F8),
        "f9" => Some(enigo::Key::F9),
        "f10" => Some(enigo::Key::F10),
        "f11" => Some(enigo::Key::F11),
        "f12" => Some(enigo::Key::F12),
        _ => None,
    }
}

fn parse_key(name: &str, line: usize) -> DesktopToolResult<ParsedKey> {
    if let Some(key) = parse_named_key(name) {
        return Ok(ParsedKey::Named(key));
    }
    let trimmed = name.trim();
    let mut chars = trimmed.chars();
    match (chars.next(), chars.next()) {
        (Some(ch), None) => Ok(ParsedKey::Char(ch)),
        _ => Err(operate_line_error(line, "key", format!("非法：不支持的按键 `{name}`"))),
    }
}

/// 已解析的按键：命名键（Control/Enter/F1 等）或单字符键。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParsedKey {
    Named(enigo::Key),
    Char(char),
}

/// 组合键按下保持时长：全部按键 Press 后等待该时长再释放，
/// 避免修饰键与普通键被系统/应用识别为两次独立点击。
const COMBO_KEY_PRESS_HOLD: std::time::Duration = std::time::Duration::from_millis(15);

/// ASCII 字符到 Windows 虚拟键（VK）的映射。组合键中的字母/数字/常见符号
/// 必须走真实按键事件才能触发系统与应用快捷键；非 ASCII 字符（如中文）返回 None。
#[cfg(target_os = "windows")]
fn char_to_vk(ch: char) -> Option<u16> {
    let lower = ch.to_ascii_lowercase();
    match lower {
        'a'..='z' => Some(0x41 + (lower as u16 - 'a' as u16)), // VK_A..VK_Z
        '0'..='9' => Some(0x30 + (lower as u16 - '0' as u16)), // VK_0..VK_9
        '-' => Some(0xBD),  // VK_OEM_MINUS
        '=' => Some(0xBB),  // VK_OEM_PLUS
        '[' => Some(0xDB),  // VK_OEM_4
        ']' => Some(0xDD),  // VK_OEM_6
        '\\' => Some(0xDC), // VK_OEM_5
        ';' => Some(0xBA),  // VK_OEM_1
        '\'' => Some(0xDE), // VK_OEM_7
        '`' => Some(0xC0),  // VK_OEM_3
        ',' => Some(0xBC),  // VK_OEM_COMMA
        '.' => Some(0xBE),  // VK_OEM_PERIOD
        '/' => Some(0xBF),  // VK_OEM_2
        ' ' => Some(0x20),  // VK_SPACE
        _ => None,
    }
}

/// 获取前台窗口线程的键盘布局，与 enigo 的 VK→scan 转换保持一致，
/// 避免 Tokio worker 线程布局与目标应用布局不一致导致符号键映射错位。
#[cfg(target_os = "windows")]
fn foreground_keyboard_layout() -> windows_sys::Win32::UI::Input::KeyboardAndMouse::HKL {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetKeyboardLayout;
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
    let foreground = unsafe { GetForegroundWindow() };
    let thread_id = unsafe { GetWindowThreadProcessId(foreground, std::ptr::null_mut()) };
    unsafe { GetKeyboardLayout(thread_id) }
}

/// 发送单字符按键。组合键（prefer_real_key=true）在 Windows 上用
/// VK→scan code→enigo.raw 注入真实按键事件；单键文本输入保持 Unicode 注入
/// （绕过输入法直接上屏，现状行为不变）。组合键遇到不可映射字符直接报错，
/// 不回退 Unicode——enigo 对 Unicode 键的 Press/Release 会各自注入一次完整
/// 文本（down+up），回退会导致字符重复输入且快捷键仍不生效。
fn send_char_key(
    enigo: &mut enigo::Enigo,
    ch: char,
    direction: enigo::Direction,
    context: &str,
    prefer_real_key: bool,
) -> DesktopToolResult<()> {
    #[cfg(target_os = "windows")]
    if prefer_real_key {
        let Some(vk) = char_to_vk(ch) else {
            return Err(DesktopToolError::internal_error(format!(
                "{context}: 不支持的按键字符 `{ch}`，组合键请使用基础键并显式携带修饰键（如 Ctrl+Shift+/ 而非 Ctrl+?）"
            )));
        };
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{MapVirtualKeyExW, MAPVK_VK_TO_VSC_EX};
        let scan = unsafe { MapVirtualKeyExW(vk as u32, MAPVK_VK_TO_VSC_EX, foreground_keyboard_layout()) };
        if scan != 0 {
            return enigo
                .raw(scan as u16, direction)
                .map_err(|err| map_input_err(err, context));
        }
        return Err(DesktopToolError::internal_error(format!(
            "{context}: 按键字符 `{ch}` 无法映射为扫描码（VK={vk:#04x}）"
        )));
    }
    enigo
        .key(enigo::Key::Unicode(ch), direction)
        .map_err(|err| map_input_err(err, context))
}

fn press_parsed_key(enigo: &mut enigo::Enigo, key: ParsedKey, prefer_real_key: bool) -> DesktopToolResult<()> {
    match key {
        ParsedKey::Named(k) => enigo.key(k, enigo::Direction::Press).map_err(|err| map_input_err(err, "key press failed")),
        ParsedKey::Char(ch) => send_char_key(enigo, ch, enigo::Direction::Press, "key press failed", prefer_real_key),
    }
}

fn release_parsed_key(enigo: &mut enigo::Enigo, key: ParsedKey, prefer_real_key: bool) -> DesktopToolResult<()> {
    match key {
        ParsedKey::Named(k) => enigo.key(k, enigo::Direction::Release).map_err(|err| map_input_err(err, "key release failed")),
        ParsedKey::Char(ch) => send_char_key(enigo, ch, enigo::Direction::Release, "key release failed", prefer_real_key),
    }
}

fn primary_monitor_bounds() -> DesktopToolResult<ScreenBounds> {
    let monitors = monitor_list()?;
    let monitor = resolve_primary_monitor(&monitors);
    let x = monitor.x().unwrap_or(0);
    let y = monitor.y().unwrap_or(0);
    let width = monitor.width().map_err(|err| DesktopToolError::internal_error(format!("read monitor width failed: {err}")))?;
    let height = monitor.height().map_err(|err| DesktopToolError::internal_error(format!("read monitor height failed: {err}")))?;
    Ok(ScreenBounds { x, y, width, height })
}

/// 按显示器 id 取坐标基准；id 为 None 时用主屏。id 越界时明确报错，不静默回退到主屏。
fn monitor_bounds_by_id(monitor_id: Option<u32>) -> DesktopToolResult<ScreenBounds> {
    let Some(id) = monitor_id else {
        return primary_monitor_bounds();
    };
    let monitors = monitor_list()?;
    let monitor = resolve_monitor_by_id(&monitors, id)
        .ok_or_else(|| DesktopToolError::invalid_params(format!("显示器不存在：monitor={id}；可用 screenshot 确认显示器编号，或省略 monitor 使用主屏")))?;
    let width = monitor.width().map_err(|err| DesktopToolError::internal_error(format!("read monitor width failed: {err}")))?;
    let height = monitor.height().map_err(|err| DesktopToolError::internal_error(format!("read monitor height failed: {err}")))?;
    Ok(ScreenBounds { x: monitor.x().unwrap_or(0), y: monitor.y().unwrap_or(0), width, height })
}

fn normalized_point_to_screen(point: &NormalizedPoint, bounds: &ScreenBounds) -> (i32, i32) {
    let max_x = bounds.width.saturating_sub(1) as f64;
    let max_y = bounds.height.saturating_sub(1) as f64;
    (bounds.x + (point.x * max_x).round() as i32, bounds.y + (point.y * max_y).round() as i32)
}

fn normalized_region_to_screen(region: &NormalizedRegion, bounds: &ScreenBounds) -> ScreenBounds {
    let width_f = bounds.width as f64;
    let height_f = bounds.height as f64;
    ScreenBounds {
        x: bounds.x + (region.x * width_f).round() as i32,
        y: bounds.y + (region.y * height_f).round() as i32,
        width: (region.width * width_f).round().max(1.0) as u32,
        height: (region.height * height_f).round().max(1.0) as u32,
    }
}

async fn sleep_duration(duration: std::time::Duration) {
    if !duration.is_zero() {
        tokio::time::sleep(duration).await;
    }
}

// ==================== 前台动作目标校验与焦点策略（C1/C2/C3/E3） ====================

/// 前台目标处理被阻断：校验不通过，或按策略抢焦点失败。
struct ForegroundBlock {
    message: String,
    focus_failed: Option<FocusFailureInfo>,
}

/// 解析前台动作声明的目标窗口：句柄精确匹配；标题按子串匹配（忽略大小写，唯一命中才算数）。
/// 找不到或命中多个时列出候选窗口标题，让模型能改对名字。
fn resolve_foreground_window(target: &ForegroundTarget) -> DesktopToolResult<WindowInfo> {
    let windows = crate::platform::list_all_windows();
    match target {
        ForegroundTarget::WindowId(id) => windows
            .into_iter()
            .find(|w| w.window_id == *id as usize)
            .ok_or_else(|| {
                DesktopToolError::invalid_params(format!(
                    "目标窗口不存在：windowId={id}（可能已关闭或被隐藏；可用 list windows 查看当前可见窗口）"
                ))
            }),
        ForegroundTarget::Title(title) => {
            let needle = title.to_lowercase();
            let matched = windows
                .iter()
                .filter(|w| !w.title.is_empty() && w.title.to_lowercase().contains(&needle))
                .collect::<Vec<_>>();
            match matched.as_slice() {
                [only] => Ok((*only).clone()),
                [] => Err(DesktopToolError::invalid_params(format!(
                    "目标窗口不存在：没有标题包含 `{title}` 的可见窗口；当前可见窗口：{}",
                    window_title_candidates(&windows)
                ))),
                many => Err(DesktopToolError::invalid_params(format!(
                    "目标窗口不唯一：标题包含 `{title}` 的窗口有 {} 个：{}；请改用更精确的标题或直接给 windowId",
                    many.len(),
                    many.iter().map(|w| format!("{}（{}）", w.title, w.window_id)).collect::<Vec<_>>().join("、")
                ))),
            }
        }
    }
}

/// 可见窗口标题候选（最多 12 条），用于报错时给模型可改的名字。
fn window_title_candidates(windows: &[WindowInfo]) -> String {
    let mut items = windows
        .iter()
        .filter(|w| !w.title.is_empty())
        .take(12)
        .map(|w| format!("{}（{}）", w.title, w.window_id))
        .collect::<Vec<_>>();
    if items.is_empty() {
        return "（无可见窗口）".to_string();
    }
    if windows.len() > items.len() {
        items.push("…".to_string());
    }
    items.join("、")
}

/// 当前前台窗口标题；无前台窗口时返回空串。
fn foreground_title(windows: &[WindowInfo]) -> String {
    windows.iter().find(|w| w.focused).map(|w| w.title.clone()).unwrap_or_default()
}

/// 抢焦点失败时的结构化现场（C3）：目标是否还在、是否可见/最小化、前后前台是谁、建议下一步。
fn build_focus_failure(window: &WindowInfo, foreground_before: String) -> FocusFailureInfo {
    let windows = crate::platform::list_all_windows();
    let alive = crate::platform::window_is_alive(window.window_id);
    let current = windows.iter().find(|w| w.window_id == window.window_id);
    let visible = current.is_some();
    let minimized = current.map(|w| w.minimized).unwrap_or(false);
    let foreground_after = foreground_title(&windows);
    let suggested_recovery = if !alive {
        vec!["open_application".to_string()]
    } else if !visible {
        vec!["unhide_app".to_string(), "activate_window".to_string()]
    } else {
        vec!["activate_window".to_string()]
    };
    FocusFailureInfo {
        target_window_id: window.window_id as u32,
        target_title: window.title.clone(),
        alive,
        visible,
        minimized,
        foreground_before,
        foreground_after,
        suggested_recovery,
    }
}

/// 按 focus 策略处理前台动作的目标窗口。
/// verify：不激活，只校验当前前台就是目标窗口，不符即阻断（避免输入静默打到别的应用）；
/// best_effort：尝试激活，失败不阻断，返回一句现场描述供步骤摘要使用；
/// strict：必须激活成功，失败即阻断并附结构化现场。
/// 敏感应用门控（J1）：窗口标题命中用户配置的黑名单子串时返回命中的关键词。
/// 空名单直接放行，不改变现有行为；匹配忽略大小写，空条目跳过。
fn blocked_app_hit<'a>(blocked: &'a [String], title: &str) -> Option<&'a str> {
    if blocked.is_empty() || title.trim().is_empty() {
        return None;
    }
    let title_lower = title.to_lowercase();
    blocked.iter().find_map(|keyword| {
        let needle = keyword.trim();
        if needle.is_empty() {
            return None;
        }
        if title_lower.contains(&needle.to_lowercase()) {
            Some(needle)
        } else {
            None
        }
    })
}

/// 门控阻断文案：说清命中了哪个关键词、该换目标还是改配置（I2）。
fn blocked_app_message(title: &str, keyword: &str) -> String {
    format!(
        "目标应用已被禁止操作：窗口「{title}」命中禁止名单「{keyword}」；请换一个目标窗口，或在设置中修改禁止名单后重试"
    )
}

async fn apply_foreground_policy(
    window_target: &Option<ForegroundTarget>,
    focus: FocusPolicy,
    blocked_apps: &[String],
) -> Result<Option<String>, ForegroundBlock> {
    let Some(target) = window_target else {
        // 未声明目标的动作直接作用于当前前台；前台命中黑名单同样阻断（J1）
        if !blocked_apps.is_empty() {
            let windows = crate::platform::list_all_windows();
            if let Some(foreground) = windows.iter().find(|w| w.focused) {
                if let Some(keyword) = blocked_app_hit(blocked_apps, &foreground.title) {
                    return Err(ForegroundBlock {
                        message: blocked_app_message(&foreground.title, keyword),
                        focus_failed: None,
                    });
                }
            }
        }
        return Ok(None);
    };
    let window = resolve_foreground_window(target).map_err(|err| ForegroundBlock { message: err.message, focus_failed: None })?;
    if let Some(keyword) = blocked_app_hit(blocked_apps, &window.title) {
        return Err(ForegroundBlock {
            message: blocked_app_message(&window.title, keyword),
            focus_failed: None,
        });
    }
    match focus {
        FocusPolicy::Verify => {
            let windows = crate::platform::list_all_windows();
            if windows.iter().any(|w| w.focused && w.window_id == window.window_id) {
                return Ok(None);
            }
            let actual = foreground_title(&windows);
            Err(ForegroundBlock {
                message: format!(
                    "目标窗口不在前台：期望「{}」（{}），当前前台为「{}」；请改用 focus=best_effort/strict 自动激活，或先执行 activate window",
                    window.title, window.window_id, actual
                ),
                focus_failed: None,
            })
        }
        FocusPolicy::BestEffort | FocusPolicy::Strict => {
            let windows = crate::platform::list_all_windows();
            let before = foreground_title(&windows);
            let window_id = window.window_id;
            // SetForegroundWindow 会阻塞轮询最多 1.5s，放阻塞线程池
            let (_, activated) = tokio::task::spawn_blocking(move || crate::platform::activate_window(window_id))
                .await
                .map_err(|err| ForegroundBlock { message: format!("激活窗口任务失败：{err}"), focus_failed: None })?;
            if activated {
                return Ok(None);
            }
            let info = build_focus_failure(&window, before);
            let summary = format!(
                "抢焦点失败：目标「{}」（{}），窗口存在={}，可见={}，最小化={}，前台「{}」->「{}」；建议 {:?}",
                info.target_title, info.target_window_id, info.alive, info.visible, info.minimized,
                info.foreground_before, info.foreground_after, info.suggested_recovery
            );
            if focus == FocusPolicy::Strict {
                Err(ForegroundBlock { message: summary, focus_failed: Some(info) })
            } else {
                Ok(Some(summary))
            }
        }
    }
}

/// 查询屏幕坐标命中的可交互元素描述（E3）：让模型确认前台点击实际点到了什么。
/// 只扫目标窗口；坐标落在空白处、窗口未暴露 UIA 或读取失败时返回 None。
fn hit_element_summary(window_id: usize, screen_x: i32, screen_y: i32) -> Option<String> {
    let bounds = primary_monitor_bounds().ok()?;
    let max_x = bounds.width.saturating_sub(1) as f64;
    let max_y = bounds.height.saturating_sub(1) as f64;
    if max_x <= 0.0 || max_y <= 0.0 {
        return None;
    }
    let elements = crate::platform::collect_window_ui_elements(
        window_id,
        bounds.x as f64,
        bounds.y as f64,
        bounds.width as f64,
        bounds.height as f64,
        false,
    );
    if elements.is_empty() {
        return None;
    }
    let nx = (screen_x - bounds.x) as f64 / max_x;
    let ny = (screen_y - bounds.y) as f64 / max_y;
    let hit = elements
        .iter()
        .find(|e| nx >= e.x && nx <= e.x + e.width && ny >= e.y && ny <= e.y + e.height)?;
    let name = if hit.name.trim().is_empty() { "(无名称)" } else { hit.name.trim() };
    Some(format!("命中元素：{}「{}」", hit.control_type, name))
}

/// 条件等待的轮询间隔：200ms 足够跟上常见界面响应，又不会把 CPU 打满。
const WAIT_UNTIL_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(200);

/// 轮询等待条件成立，返回成立时的现场描述；超时返回明确失败并给出下一步（D2）。
async fn wait_until(condition: &WaitCondition, timeout: std::time::Duration) -> DesktopToolResult<String> {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        if let Some(description) = wait_condition_met(condition) {
            return Ok(description);
        }
        if std::time::Instant::now() >= deadline {
            return Err(DesktopToolError::invalid_params(format!(
                "等待超时（{:.1}s）：{}；可先执行 screenshot 观察当前界面，或改用更大的 timeout",
                timeout.as_secs_f64(),
                wait_condition_missing(condition)
            )));
        }
        sleep_duration(WAIT_UNTIL_POLL_INTERVAL).await;
    }
}

/// 条件尚未成立时的描述，用于超时文案。
fn wait_condition_missing(condition: &WaitCondition) -> String {
    match condition {
        WaitCondition::Window { title } => format!("未出现标题包含「{title}」的可见窗口"),
        WaitCondition::Element { name, .. } => format!("未出现名称包含「{name}」的可交互元素"),
    }
}

/// 条件是否成立；成立时返回现场描述。
fn wait_condition_met(condition: &WaitCondition) -> Option<String> {
    match condition {
        WaitCondition::Window { title } => {
            let needle = title.to_lowercase();
            crate::platform::list_all_windows()
                .into_iter()
                .find(|w| !w.title.is_empty() && w.title.to_lowercase().contains(&needle))
                .map(|w| format!("window appeared: {}（{}）", w.title, w.window_id))
        }
        WaitCondition::Element { name, window_target } => {
            let needle = name.to_lowercase();
            let window_id = match window_target {
                Some(target) => resolve_foreground_window(target).ok()?.window_id,
                None => foreground_window_id()?,
            };
            let bounds = primary_monitor_bounds().ok()?;
            let elements = crate::platform::collect_window_ui_elements(
                window_id,
                bounds.x as f64,
                bounds.y as f64,
                bounds.width as f64,
                bounds.height as f64,
                false,
            );
            elements
                .into_iter()
                .find(|e| !e.name.trim().is_empty() && e.name.to_lowercase().contains(&needle))
                .map(|e| format!("element appeared: {}「{}」", e.control_type, e.name))
        }
    }
}

/// 当前前台窗口 id；无前台窗口时返回 None。
fn foreground_window_id() -> Option<usize> {
    crate::platform::list_all_windows().into_iter().find(|w| w.focused).map(|w| w.window_id)
}

/// 前台坐标点击后附带命中元素描述（E3）；无法定位时返回空串，不影响步骤结果。
fn mouse_hit_note(target: &NormalizedPoint, monitor: Option<u32>) -> String {
    let Ok(bounds) = monitor_bounds_by_id(monitor) else {
        return String::new();
    };
    let Some(window_id) = foreground_window_id() else {
        return String::new();
    };
    let (x, y) = normalized_point_to_screen(target, &bounds);
    hit_element_summary(window_id, x, y)
        .map(|text| format!("，{text}"))
        .unwrap_or_default()
}

async fn execute_mouse_click(enigo: &mut enigo::Enigo, button: OperateMouseButton, target: &NormalizedPoint, monitor: Option<u32>, repeat: u32, delay: std::time::Duration, pre_delay: std::time::Duration, press: std::time::Duration) -> DesktopToolResult<()> {
    sleep_duration(pre_delay).await;
    let bounds = monitor_bounds_by_id(monitor)?;
    let (x, y) = normalized_point_to_screen(target, &bounds);
    enigo.move_mouse(x, y, enigo::Coordinate::Abs).map_err(|err| map_input_err(err, "move mouse failed"))?;
    let mapped = map_mouse_button(button);
    for idx in 0..repeat {
        if press.is_zero() {
            enigo.button(mapped, enigo::Direction::Click).map_err(|err| map_input_err(err, "mouse click failed"))?;
        } else {
            enigo.button(mapped, enigo::Direction::Press).map_err(|err| map_input_err(err, "mouse down failed"))?;
            sleep_duration(press).await;
            enigo.button(mapped, enigo::Direction::Release).map_err(|err| map_input_err(err, "mouse up failed"))?;
        }
        if idx + 1 < repeat {
            sleep_duration(delay).await;
        }
    }
    Ok(())
}

async fn execute_mouse_move(enigo: &mut enigo::Enigo, target: &NormalizedPoint, monitor: Option<u32>, pre_delay: std::time::Duration) -> DesktopToolResult<()> {
    sleep_duration(pre_delay).await;
    let bounds = monitor_bounds_by_id(monitor)?;
    let (x, y) = normalized_point_to_screen(target, &bounds);
    enigo.move_mouse(x, y, enigo::Coordinate::Abs).map_err(|err| map_input_err(err, "mouse move failed"))
}

/// 拖拽插值帧间隔：约 60fps。瞬移式移动会被多数应用识别成「点了一下」而不是拖拽，
/// 必须逐帧推送中间位置。
const MOUSE_DRAG_FRAME_INTERVAL: std::time::Duration = std::time::Duration::from_millis(16);
/// 未显式给 duration 时，按起止距离推算拖拽时长（秒），并夹在下列区间内。
const MOUSE_DRAG_MIN_DURATION_SECS: f64 = 0.1;
const MOUSE_DRAG_MAX_DURATION_SECS: f64 = 0.5;

fn default_drag_duration(x1: i32, y1: i32, x2: i32, y2: i32) -> std::time::Duration {
    let distance = (((x2 - x1) as f64).powi(2) + ((y2 - y1) as f64).powi(2)).sqrt();
    let secs = (distance / 2000.0).clamp(MOUSE_DRAG_MIN_DURATION_SECS, MOUSE_DRAG_MAX_DURATION_SECS);
    std::time::Duration::from_secs_f64(secs)
}

async fn execute_mouse_drag(enigo: &mut enigo::Enigo, button: OperateMouseButton, from: &NormalizedPoint, to: &NormalizedPoint, monitor: Option<u32>, duration: Option<std::time::Duration>, pre_delay: std::time::Duration) -> DesktopToolResult<()> {
    sleep_duration(pre_delay).await;
    let bounds = monitor_bounds_by_id(monitor)?;
    let (x1, y1) = normalized_point_to_screen(from, &bounds);
    let (x2, y2) = normalized_point_to_screen(to, &bounds);
    let mapped = map_mouse_button(button);
    enigo.move_mouse(x1, y1, enigo::Coordinate::Abs).map_err(|err| map_input_err(err, "move mouse failed"))?;
    enigo.button(mapped, enigo::Direction::Press).map_err(|err| map_input_err(err, "mouse down failed"))?;
    let total = duration.unwrap_or_else(|| default_drag_duration(x1, y1, x2, y2));
    let steps = ((total.as_secs_f64() / MOUSE_DRAG_FRAME_INTERVAL.as_secs_f64()).round() as u32).max(1);
    for step in 1..=steps {
        let ratio = step as f64 / steps as f64;
        let x = (x1 as f64 + (x2 - x1) as f64 * ratio).round() as i32;
        let y = (y1 as f64 + (y2 - y1) as f64 * ratio).round() as i32;
        if let Err(err) = enigo.move_mouse(x, y, enigo::Coordinate::Abs) {
            // 中途失败必须主动松开，否则按键残留按下状态，后续操作全部异常
            let _ = enigo.button(mapped, enigo::Direction::Release);
            return Err(map_input_err(err, "drag move failed"));
        }
        sleep_duration(MOUSE_DRAG_FRAME_INTERVAL).await;
    }
    enigo.button(mapped, enigo::Direction::Release).map_err(|err| map_input_err(err, "mouse up failed"))?;
    Ok(())
}

/// 按下或释放指定鼠标键（不移动光标）：用于长按菜单、画笔等需要跨步骤精确时序的操作。
async fn execute_mouse_button_state(enigo: &mut enigo::Enigo, button: OperateMouseButton, pressed: bool, pre_delay: std::time::Duration) -> DesktopToolResult<()> {
    sleep_duration(pre_delay).await;
    let mapped = map_mouse_button(button);
    let direction = if pressed { enigo::Direction::Press } else { enigo::Direction::Release };
    let context = if pressed { "mouse down failed" } else { "mouse up failed" };
    enigo.button(mapped, direction).map_err(|err| map_input_err(err, context))
}

async fn execute_mouse_scroll(enigo: &mut enigo::Enigo, direction: i32, repeat: u32, delay: std::time::Duration, pre_delay: std::time::Duration) -> DesktopToolResult<()> {
    sleep_duration(pre_delay).await;
    for idx in 0..repeat {
        enigo.scroll(direction, enigo::Axis::Vertical).map_err(|err| map_input_err(err, "mouse scroll failed"))?;
        if idx + 1 < repeat {
            sleep_duration(delay).await;
        }
    }
    Ok(())
}

async fn execute_key_action(enigo: &mut enigo::Enigo, keys: &[String], line: usize, repeat: u32, delay: std::time::Duration, pre_delay: std::time::Duration, press: std::time::Duration) -> DesktopToolResult<()> {
    sleep_duration(pre_delay).await;
    let parsed = keys.iter().map(|key| parse_key(key, line)).collect::<DesktopToolResult<Vec<_>>>()?;
    for idx in 0..repeat {
        if parsed.len() == 1 && press.is_zero() {
            // 单键点击：命名键直接 tap，字符键走 Unicode 注入（输入场景直接上屏）
            match parsed[0] {
                ParsedKey::Named(key) => enigo.key(key, enigo::Direction::Click).map_err(|err| map_input_err(err, "key tap failed"))?,
                ParsedKey::Char(ch) => send_char_key(enigo, ch, enigo::Direction::Click, "key tap failed", false)?,
            }
        } else {
            // 组合键（或长按）：字符键必须注入真实按键事件，否则系统/应用快捷键不识别。
            // press 阶段任一键失败时，主动逆序释放已按下的键，避免修饰键残留（不依赖 Enigo Drop 兜底）。
            for (pressed_idx, key) in parsed.iter().enumerate() {
                if let Err(err) = press_parsed_key(enigo, *key, true) {
                    for released in parsed[..pressed_idx].iter().rev() {
                        let _ = release_parsed_key(enigo, *released, true);
                    }
                    return Err(err);
                }
            }
            let hold = if press.is_zero() { COMBO_KEY_PRESS_HOLD } else { press };
            sleep_duration(hold).await;
            for key in parsed.iter().rev() {
                release_parsed_key(enigo, *key, true)?;
            }
        }
        if idx + 1 < repeat {
            sleep_duration(delay).await;
        }
    }
    Ok(())
}

async fn execute_text_action(enigo: &mut enigo::Enigo, text: &str, repeat: u32, delay: std::time::Duration, pre_delay: std::time::Duration) -> DesktopToolResult<()> {
    sleep_duration(pre_delay).await;
    for idx in 0..repeat {
        execute_text_once(enigo, text).await?;
        if idx + 1 < repeat {
            sleep_duration(delay).await;
        }
    }
    Ok(())
}

/// 剪贴板粘贴的触发条件：含非 ASCII（绕开中文 IME）、含换行或超过 100 字符（避免 UWP/RichEdit 逐字丢字）。
#[cfg(target_os = "windows")]
fn should_paste_via_clipboard(text: &str) -> bool {
    contains_non_ascii(text) || text.contains('\n') || text.chars().count() > 100
}

/// 单次 text 注入。Windows 上含非 ASCII 字符、含换行或超过 100 字符时改走剪贴板粘贴：
/// enigo 的 KEYEVENTF_UNICODE（VK_PACKET）注入会被中文 IME 拦截进
/// composition 缓冲，与 Enter 交替时提交顺序错乱；剪贴板粘贴完全绕开
/// 键盘事件与 IME。长文本与含换行文本逐字注入在 UWP/RichEdit 控件里会丢字，
/// 同样走剪贴板。短纯 ASCII 保持 enigo 注入（避免无谓的剪贴板覆盖）。
#[cfg(target_os = "windows")]
async fn execute_text_once(enigo: &mut enigo::Enigo, text: &str) -> DesktopToolResult<()> {
    if should_paste_via_clipboard(text) {
        let previous = read_clipboard_unicode_text();
        write_clipboard_unicode_text(text)?;
        let paste_result = (|| -> DesktopToolResult<()> {
            enigo
                .key(enigo::Key::Control, enigo::Direction::Press)
                .map_err(|err| map_input_err(err, "text paste failed"))?;
            // 'v' 走真实扫描码注入，确保系统识别 Ctrl+V 组合
            send_char_key(enigo, 'v', enigo::Direction::Click, "text paste failed", true)?;
            enigo
                .key(enigo::Key::Control, enigo::Direction::Release)
                .map_err(|err| map_input_err(err, "text paste failed"))?;
            Ok(())
        })();
        // Ctrl+V 是异步注入：事件进入系统队列后目标窗口还需时间处理粘贴。
        // 立即恢复剪贴板会抢跑，导致目标窗口粘贴到恢复后的旧值。
        // 等待粘贴处理完成（约 150ms 足够记事本等标准控件完成 WM_PASTE）。
        sleep_duration(std::time::Duration::from_millis(150)).await;
        restore_clipboard_unicode_text(previous);
        return paste_result;
    }
    enigo.text(text).map_err(|err| map_input_err(err, "text input failed"))
}

#[cfg(not(target_os = "windows"))]
async fn execute_text_once(enigo: &mut enigo::Enigo, text: &str) -> DesktopToolResult<()> {
    enigo.text(text).map_err(|err| map_input_err(err, "text input failed"))
}

/// 是否包含非 ASCII 字符：Windows 分支据此决定走剪贴板粘贴还是 enigo 注入。
#[cfg(target_os = "windows")]
fn contains_non_ascii(text: &str) -> bool {
    text.chars().any(|c| !c.is_ascii())
}

/// 读取剪贴板文本（CF_UNICODETEXT）；无文本格式时返回 None。
#[cfg(target_os = "windows")]
fn read_clipboard_unicode_text() -> Option<String> {
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, GetClipboardData, OpenClipboard,
    };
    use windows_sys::Win32::System::Memory::{GlobalLock, GlobalUnlock};
    use windows_sys::Win32::System::Ole::CF_UNICODETEXT;
    unsafe {
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            return None;
        }
        let handle = GetClipboardData(CF_UNICODETEXT as u32);
        if handle.is_null() {
            CloseClipboard();
            return None;
        }
        let ptr = GlobalLock(handle);
        if ptr.is_null() {
            CloseClipboard();
            return None;
        }
        let mut len = 0usize;
        while *ptr.cast::<u16>().add(len) != 0 {
            len += 1;
        }
        let text = String::from_utf16_lossy(std::slice::from_raw_parts(ptr.cast::<u16>(), len));
        GlobalUnlock(handle);
        CloseClipboard();
        Some(text)
    }
}

/// 写入剪贴板文本（CF_UNICODETEXT，UTF-16 + 结尾 NUL）。
#[cfg(target_os = "windows")]
fn write_clipboard_unicode_text(text: &str) -> DesktopToolResult<()> {
    use windows_sys::Win32::Foundation::GlobalFree;
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows_sys::Win32::System::Memory::{
        GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE,
    };
    use windows_sys::Win32::System::Ole::CF_UNICODETEXT;
    unsafe {
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            return Err(DesktopToolError::internal_error("open clipboard failed"));
        }
        if EmptyClipboard() == 0 {
            CloseClipboard();
            return Err(DesktopToolError::internal_error("empty clipboard failed"));
        }
        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let bytes = wide.len() * std::mem::size_of::<u16>();
        let handle = GlobalAlloc(GMEM_MOVEABLE, bytes);
        if handle.is_null() {
            CloseClipboard();
            return Err(DesktopToolError::internal_error("global alloc failed"));
        }
        let ptr = GlobalLock(handle);
        if ptr.is_null() {
            GlobalFree(handle);
            CloseClipboard();
            return Err(DesktopToolError::internal_error("global lock failed"));
        }
        std::ptr::copy_nonoverlapping(wide.as_ptr().cast::<u8>(), ptr.cast::<u8>(), bytes);
        GlobalUnlock(handle);
        if SetClipboardData(CF_UNICODETEXT as u32, handle).is_null() {
            GlobalFree(handle);
            CloseClipboard();
            return Err(DesktopToolError::internal_error("set clipboard data failed"));
        }
        CloseClipboard();
    }
    Ok(())
}

/// 恢复剪贴板：有原文本则写回；无文本格式则清空（原非文本内容在
/// 写入时已被 EmptyClipboard 清除，此为计划内声明的限制）。
#[cfg(target_os = "windows")]
fn restore_clipboard_unicode_text(previous: Option<String>) {
    match previous {
        Some(text) => {
            let _ = write_clipboard_unicode_text(&text);
        }
        None => unsafe {
            use windows_sys::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard};
            if OpenClipboard(std::ptr::null_mut()) != 0 {
                EmptyClipboard();
                CloseClipboard();
            }
        },
    }
}

/// 生成 operate 截图默认保存路径：{screenshots_root}/operate_{毫秒时间戳}.webp
fn default_operate_screenshot_path(screenshots_root: &std::path::Path) -> String {
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or_default();
    screenshots_root
        .join(format!("operate_{ms}.webp"))
        .to_string_lossy()
        .to_string()
}

async fn execute_screenshot_action(
    mode: &ScreenshotModeSpec,
    save_path: Option<String>,
    quality: f32,
    screenshots_root: &std::path::Path,
    include_base64: bool,
    elements: bool,
    include_text: bool,
) -> DesktopToolResult<(ScreenshotResponse, String, Option<Vec<UiElementInfo>>)> {
    let save_path = save_path.or_else(|| Some(default_operate_screenshot_path(screenshots_root)));
    let request = ScreenshotRequest {
        mode: match mode {
            ScreenshotModeSpec::Desktop | ScreenshotModeSpec::FocusedWindow | ScreenshotModeSpec::WindowId(_) => ScreenshotMode::Desktop,
            ScreenshotModeSpec::Region(_) => ScreenshotMode::Region,
            ScreenshotModeSpec::Monitor(_) => ScreenshotMode::Monitor,
        },
        monitor_id: match mode {
            ScreenshotModeSpec::Monitor(id) => Some(*id),
            _ => None,
        },
        region: match mode {
            ScreenshotModeSpec::Region(region) => {
                let bounds = primary_monitor_bounds()?;
                Some(normalized_region_to_screen(region, &bounds))
            }
            _ => None,
        },
        save_path,
        webp_quality: quality,
        include_base64,
    };
    let result = match mode {
        ScreenshotModeSpec::Desktop | ScreenshotModeSpec::Region(_) | ScreenshotModeSpec::Monitor(_) => run_screenshot_tool(request).await?,
        ScreenshotModeSpec::FocusedWindow => run_capture_window_tool(request, None)?,
        ScreenshotModeSpec::WindowId(window_id) => run_capture_window_tool(request, Some(*window_id))?,
    };
    let mode_name = match mode {
        ScreenshotModeSpec::Desktop => "desktop",
        ScreenshotModeSpec::FocusedWindow => "focused_window",
        ScreenshotModeSpec::WindowId(_) => "window_id",
        ScreenshotModeSpec::Region(_) => "region",
        ScreenshotModeSpec::Monitor(_) => "monitor",
    }
    .to_string();

    // elements=true：扫描可交互元素树（当前 Windows 实现；其他平台返回空并在 summary 提示）。
    // text=true 时额外纳入非交互文本标签，模型可借「用户名: [输入框]」这类上下文定位目标（E2）。
    // UIA 遍历是同步阻塞调用（数百 ms），放到阻塞线程池执行，避免占用 Tokio 工作线程。
    let mut tree = if elements {
        let mode_for_scan = mode.clone();
        Some(
            tokio::task::spawn_blocking(move || collect_ui_tree_for_mode(&mode_for_scan, include_text))
                .await
                .unwrap_or_default(),
        )
    } else {
        None
    };

    // 为元素分配快照引用编号（app 动作 el= 的取值），并登记最近一次快照供动作解析核对
    if let Some(elems) = tree.as_mut() {
        for (idx, elem) in elems.iter_mut().enumerate() {
            elem.element_ref = Some(idx as u32 + 1);
        }
        store_element_tree(elems);
    }

    Ok((result, mode_name, tree))
}

// ==================== app 后台动作（元素 ref 注册 + 分发） ====================

/// 最近一次 operate 截图返回的元素快照（含 ref 编号），供 app 动作 el= 解析与核对。
static LAST_ELEMENT_TREE: std::sync::Mutex<Vec<UiElementInfo>> = std::sync::Mutex::new(Vec::new());

fn store_element_tree(tree: &[UiElementInfo]) {
    if let Ok(mut slot) = LAST_ELEMENT_TREE.lock() {
        *slot = tree.to_vec();
    }
}

/// 解析 el=<n>：在最近快照中定位元素，校验所属窗口，并换算为该窗口内的扫描序号。
fn resolve_element_ref(el: u32, window_id: u32) -> DesktopToolResult<(usize, String, String)> {
    let tree = LAST_ELEMENT_TREE
        .lock()
        .map_err(|_| DesktopToolError::internal_error("元素快照注册表被占用"))?;
    let pos = tree
        .iter()
        .position(|e| e.element_ref == Some(el))
        .ok_or_else(|| {
            DesktopToolError::invalid_params(format!("el={el} 不存在：请先执行 screenshot（elements=true）获取元素引用"))
        })?;
    let entry = &tree[pos];
    if entry.window_id != window_id {
        return Err(DesktopToolError::invalid_params(format!(
            "el={el} 属于窗口 {}，与目标窗口 {window_id} 不一致",
            entry.window_id
        )));
    }
    let ordinal = tree[..pos].iter().filter(|e| e.window_id == entry.window_id).count();
    Ok((ordinal, entry.control_type.clone(), entry.name.clone()))
}

async fn build_app_target(window_id: u32, target: AppScriptTarget, monitor: Option<u32>) -> DesktopToolResult<AppTarget> {
    match target {
        AppScriptTarget::Element(el) => {
            let (ordinal, control_type, name) = resolve_element_ref(el, window_id)?;
            Ok(AppTarget::Element { el, ordinal, control_type, name })
        }
        AppScriptTarget::Point(point) => {
            let bounds = monitor_bounds_by_id(monitor)?;
            let (x, y) = normalized_point_to_screen(&point, &bounds);
            Ok(AppTarget::Point { screen_x: x, screen_y: y })
        }
    }
}

async fn execute_app_click(window_id: u32, target: AppScriptTarget, monitor: Option<u32>, repeat: u32, dblclick: bool, pre_delay: std::time::Duration) -> DesktopToolResult<&'static str> {
    sleep_duration(pre_delay).await;
    let app_target = build_app_target(window_id, target, monitor).await?;
    let window_id = window_id as usize;
    // UIA 重扫与 pattern 调用是同步阻塞 COM 调用，放阻塞线程池执行
    tokio::task::spawn_blocking(move || crate::platform::app_click(window_id, &app_target, repeat, dblclick))
        .await
        .map_err(|err| DesktopToolError::internal_error(format!("app click task failed: {err}")))?
        .map_err(DesktopToolError::invalid_params)
}

async fn execute_app_set_value(window_id: u32, el: u32, text: String, pre_delay: std::time::Duration) -> DesktopToolResult<&'static str> {
    sleep_duration(pre_delay).await;
    let (ordinal, control_type, name) = resolve_element_ref(el, window_id)?;
    let app_target = AppTarget::Element { el, ordinal, control_type, name };
    let window_id = window_id as usize;
    tokio::task::spawn_blocking(move || crate::platform::app_set_value(window_id, &app_target, &text))
        .await
        .map_err(|err| DesktopToolError::internal_error(format!("app setvalue task failed: {err}")))?
        .map_err(DesktopToolError::invalid_params)
}

async fn execute_app_get_value(window_id: u32, el: u32) -> DesktopToolResult<String> {
    let (ordinal, control_type, name) = resolve_element_ref(el, window_id)?;
    let app_target = AppTarget::Element { el, ordinal, control_type, name };
    let window_id = window_id as usize;
    tokio::task::spawn_blocking(move || crate::platform::app_get_value(window_id, &app_target))
        .await
        .map_err(|err| DesktopToolError::internal_error(format!("app getvalue task failed: {err}")))?
        .map_err(DesktopToolError::invalid_params)
}

async fn execute_app_scroll(window_id: u32, target: AppScriptTarget, monitor: Option<u32>, up: bool, repeat: u32, delay: std::time::Duration, pre_delay: std::time::Duration) -> DesktopToolResult<&'static str> {
    sleep_duration(pre_delay).await;
    let app_target = build_app_target(window_id, target, monitor).await?;
    let window_id = window_id as usize;
    let mut method = "scrollpattern";
    for idx in 0..repeat {
        if idx > 0 {
            sleep_duration(delay).await;
        }
        let app_target = app_target.clone();
        method = tokio::task::spawn_blocking(move || crate::platform::app_scroll(window_id, &app_target, up, true, 1))
            .await
            .map_err(|err| DesktopToolError::internal_error(format!("app scroll task failed: {err}")))?
            .map_err(DesktopToolError::invalid_params)?;
    }
    Ok(method)
}

async fn execute_app_key(window_id: u32, keys: &[String], repeat: u32, delay: std::time::Duration, pre_delay: std::time::Duration) -> DesktopToolResult<&'static str> {
    sleep_duration(pre_delay).await;
    let window_id = window_id as usize;
    let keys = keys.to_vec();
    let mut method = "postmessage";
    for idx in 0..repeat {
        if idx > 0 {
            sleep_duration(delay).await;
        }
        method = tokio::task::spawn_blocking({
            let keys = keys.clone();
            move || crate::platform::app_key(window_id, &keys, 1)
        })
        .await
        .map_err(|err| DesktopToolError::internal_error(format!("app key task failed: {err}")))?
        .map_err(DesktopToolError::invalid_params)?;
    }
    Ok(method)
}

/// 分发 app 动作；返回 (动作名, 实际投递方式, 附加摘要)。
/// 附加摘要：getvalue 返回读到的值；写/点/滚/键动作返回动作后的内部焦点控件描述（focus=Type('name')），
/// 让模型不重截就能确认焦点去向。post_delay 在动作完成后统一等待。
async fn execute_app_action(window_id: u32, action: AppScriptAction, post_delay: std::time::Duration) -> DesktopToolResult<(&'static str, &'static str, Option<String>)> {
    let (verb, method) = match action {
        AppScriptAction::Click { target, monitor, repeat, dblclick, pre_delay } => {
            let method = execute_app_click(window_id, target, monitor, repeat, dblclick, pre_delay).await?;
            ("click", method)
        }
        AppScriptAction::SetValue { el, text, pre_delay } => {
            let method = execute_app_set_value(window_id, el, text, pre_delay).await?;
            ("setvalue", method)
        }
        AppScriptAction::GetValue { el } => {
            let value = execute_app_get_value(window_id, el).await?;
            return Ok(("getvalue", "valuepattern", Some(format!("value={value:?}"))));
        }
        AppScriptAction::ScrollUp { target, monitor, repeat, delay, pre_delay } => {
            let method = execute_app_scroll(window_id, target, monitor, true, repeat, delay, pre_delay).await?;
            ("scroll_up", method)
        }
        AppScriptAction::ScrollDown { target, monitor, repeat, delay, pre_delay } => {
            let method = execute_app_scroll(window_id, target, monitor, false, repeat, delay, pre_delay).await?;
            ("scroll_down", method)
        }
        AppScriptAction::Key { keys, repeat, delay, pre_delay } => {
            let method = execute_app_key(window_id, &keys, repeat, delay, pre_delay).await?;
            ("key", method)
        }
    };
    sleep_duration(post_delay).await;
    let focus = query_focus_summary(window_id).await;
    Ok((verb, method, focus.map(|(t, n)| format!("focus={t}('{n}')"))))
}

/// 动作完成后查询目标窗口的内部焦点控件；仅对修改型动作有意义，失败静默为 None。
async fn query_focus_summary(window_id: u32) -> Option<(String, String)> {
    let window_id = window_id as usize;
    tokio::task::spawn_blocking(move || crate::platform::app_focus_summary(window_id))
        .await
        .ok()
        .and_then(|result| result.ok())
        .flatten()
}

/// 元素矩形与归一化 region 矩形是否有交集（region 为相对主屏 0~1 归一化）。
fn element_intersects_region(e: &UiElementInfo, rx0: f64, ry0: f64, rx1: f64, ry1: f64) -> bool {
    let (ex0, ey0) = (e.x, e.y);
    let (ex1, ey1) = (e.x + e.width, e.y + e.height);
    ex0 < rx1 && ex1 > rx0 && ey0 < ry1 && ey1 > ry0
}

/// 按截图模式扫描可交互元素树：focused_window 只扫聚焦窗口，desktop 扫全部可见窗口，
/// region 只返回与截图区域相交窗口的元素（元素矩形与 region 有交集才保留）。
fn collect_ui_tree_for_mode(mode: &ScreenshotModeSpec, include_text: bool) -> Vec<UiElementInfo> {
    // Monitor 模式的坐标基准是目标显示器；其余模式沿用主屏。
    let bounds = match mode {
        ScreenshotModeSpec::Monitor(id) => match monitor_bounds_by_id(Some(*id)) {
            Ok(b) => b,
            Err(_) => return Vec::new(),
        },
        _ => match primary_monitor_bounds() {
            Ok(b) => b,
            Err(_) => return Vec::new(),
        },
    };
    let origin_x = bounds.x as f64;
    let origin_y = bounds.y as f64;
    let primary_width = bounds.width as f64;
    let primary_height = bounds.height as f64;

    let windows = match window_list() {
        Ok(w) => w,
        Err(_) => return Vec::new(),
    };
    // region 归一化矩形（相对主屏），用于窗口级与元素级过滤
    let region_rect = match mode {
        ScreenshotModeSpec::Region(region) => Some((
            region.x,
            region.y,
            region.x + region.width,
            region.y + region.height,
        )),
        _ => None,
    };
    let targets: Vec<(usize, String)> = match mode {
        ScreenshotModeSpec::WindowId(window_id) => {
            let title = windows
                .iter()
                .find(|w| w.id().ok() == Some(*window_id))
                .map(|w| w.title().unwrap_or_default())
                .unwrap_or_default();
            vec![(*window_id as usize, title)]
        }
        ScreenshotModeSpec::FocusedWindow => windows
            .iter()
            .filter(|w| w.is_focused().unwrap_or(false))
            .map(|w| (w.id().unwrap_or(0) as usize, w.title().unwrap_or_default()))
            .collect(),
        ScreenshotModeSpec::Desktop => windows
            .iter()
            .map(|w| (w.id().unwrap_or(0) as usize, w.title().unwrap_or_default()))
            .collect(),
        ScreenshotModeSpec::Monitor(_) => windows
            .iter()
            // 只扫与该显示器矩形相交的窗口，避免为副屏截图时扫进主屏的全部窗口
            .filter(|w| {
                let (wx0, wy0) = (w.x().unwrap_or(0), w.y().unwrap_or(0));
                let (wx1, wy1) = (wx0 + w.width().unwrap_or(0) as i32, wy0 + w.height().unwrap_or(0) as i32);
                wx0 < bounds.x + bounds.width as i32 && wx1 > bounds.x && wy0 < bounds.y + bounds.height as i32 && wy1 > bounds.y
            })
            .map(|w| (w.id().unwrap_or(0) as usize, w.title().unwrap_or_default()))
            .collect(),
        ScreenshotModeSpec::Region(_) => windows
            .iter()
            // 窗口级过滤：窗口矩形与 region 有交集才扫，减少无用窗口的 UIA 遍历
            .filter(|w| {
                let Some((rx0, ry0, rx1, ry1)) = region_rect else { return false };
                let (wx0, wy0) = ((w.x().unwrap_or(0) - bounds.x) as f64 / primary_width, (w.y().unwrap_or(0) - bounds.y) as f64 / primary_height);
                let (wx1, wy1) = (
                    (w.x().unwrap_or(0) + w.width().unwrap_or(0) as i32 - bounds.x) as f64 / primary_width,
                    (w.y().unwrap_or(0) + w.height().unwrap_or(0) as i32 - bounds.y) as f64 / primary_height,
                );
                wx0 < rx1 && wx1 > rx0 && wy0 < ry1 && wy1 > ry0
            })
            .map(|w| (w.id().unwrap_or(0) as usize, w.title().unwrap_or_default()))
            .collect(),
    };
    let mut elements = collect_ui_tree_for_windows(&targets, origin_x, origin_y, primary_width, primary_height, include_text);
    if let Some((rx0, ry0, rx1, ry1)) = region_rect {
        // 元素级过滤：元素矩形与 region 有交集才保留（region 截图区域之外的元素不返回）
        elements.retain(|e| element_intersects_region(e, rx0, ry0, rx1, ry1));
    }
    elements
}

#[cfg(test)]
mod operate_actions_tests {
    use super::*;

    #[test]
    fn blocked_app_hit_should_match_case_insensitively_and_skip_empty_entries() {
        let blocked = vec!["密码".to_string(), "  ".to_string(), "Settings".to_string()];
        // 中文子串命中
        assert_eq!(blocked_app_hit(&blocked, "1Password 密码管理器"), Some("密码"));
        // 英文忽略大小写命中
        assert_eq!(blocked_app_hit(&blocked, "windows settings"), Some("Settings"));
        // 未命中
        assert_eq!(blocked_app_hit(&blocked, "记事本"), None);
        // 空标题不拦截（避免误伤无标题窗口）
        assert_eq!(blocked_app_hit(&blocked, "   "), None);
        // 空名单不改变现有行为
        let empty: Vec<String> = Vec::new();
        assert_eq!(blocked_app_hit(&empty, "1Password 密码管理器"), None);
    }

    #[test]
    fn region_tree_should_filter_out_of_region_elements() {
        // region = @0.16,0.04,0.36,0.9（归一化矩形 x:0.16~0.52, y:0.04~0.94）
        let region = ScreenshotModeSpec::Region(NormalizedRegion { x: 0.16, y: 0.04, width: 0.36, height: 0.9 });
        let all = vec![
            // region 内
            UiElementInfo { window_id: 1, window_title: "in".into(), control_type: "Button".into(), name: "in".into(), x: 0.3, y: 0.5, width: 0.05, height: 0.05, focused: false, element_ref: None },
            // 完全在 region 外（任务栏 y=0.958 场景）
            UiElementInfo { window_id: 2, window_title: "taskbar".into(), control_type: "Button".into(), name: "taskbar".into(), x: 0.3, y: 0.958, width: 0.05, height: 0.03, focused: false, element_ref: None },
            // x 越界（Chrome 场景，y 高达 4.x）
            UiElementInfo { window_id: 3, window_title: "chrome".into(), control_type: "Button".into(), name: "chrome".into(), x: 0.3, y: 4.2, width: 0.05, height: 0.05, focused: false, element_ref: None },
            // 部分相交：矩形左边缘在 region 内，右边缘超出
            UiElementInfo { window_id: 4, window_title: "partial".into(), control_type: "Edit".into(), name: "partial".into(), x: 0.4, y: 0.5, width: 0.3, height: 0.05, focused: false, element_ref: None },
            // 负坐标（PAI 窗口主屏外元素）
            UiElementInfo { window_id: 5, window_title: "neg".into(), control_type: "Button".into(), name: "neg".into(), x: -0.2, y: 0.5, width: 0.05, height: 0.05, focused: false, element_ref: None },
        ];
        let kept: Vec<&UiElementInfo> = all
            .iter()
            .filter(|e| element_intersects_region(e, 0.16, 0.04, 0.52, 0.94))
            .collect();
        let names: Vec<&str> = kept.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["in", "partial"]);
        let _ = region;
    }

    #[test]
    fn parse_key_should_keep_named_key_as_named() {
        assert_eq!(parse_key("Control", 1).unwrap(), ParsedKey::Named(enigo::Key::Control));
        assert_eq!(parse_key("Enter", 1).unwrap(), ParsedKey::Named(enigo::Key::Return));
        assert_eq!(parse_key("F5", 1).unwrap(), ParsedKey::Named(enigo::Key::F5));
    }

    #[test]
    fn parse_key_should_treat_single_char_as_char() {
        assert_eq!(parse_key("L", 1).unwrap(), ParsedKey::Char('L'));
        assert_eq!(parse_key("a", 1).unwrap(), ParsedKey::Char('a'));
        assert_eq!(parse_key("0", 1).unwrap(), ParsedKey::Char('0'));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn contains_non_ascii_should_detect_non_ascii_text() {
        assert!(!contains_non_ascii("hello world 123"));
        assert!(contains_non_ascii("派蒙和旅行者"));
        assert!(contains_non_ascii("中文标点「」"));
        assert!(contains_non_ascii("emoji \u{1F600}"));
        assert!(!contains_non_ascii(""));
    }

    #[test]
    fn parse_key_should_reject_multi_char_unknown() {
        let err = parse_key("ab", 1).unwrap_err();
        assert!(err.message.contains("不支持的按键"));
    }

    #[test]
    fn key_combo_should_parse_mixed_named_and_char() {
        // 组合键 = 命名键 + 字符键，字符键必须能被识别为 Char，
        // 才能在后端注入真实按键事件触发快捷键
        let action = parse_script(&OperateRequest { script: "key Control+L".to_string(), timeout_ms: None })
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        match action {
            DesktopScriptAction::Key { keys, .. } => {
                let parsed = keys.iter().map(|key| parse_key(key, 1)).collect::<DesktopToolResult<Vec<_>>>().unwrap();
                assert_eq!(parsed, vec![ParsedKey::Named(enigo::Key::Control), ParsedKey::Char('L')]);
            }
            _ => panic!("expected key action"),
        }
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn char_to_vk_should_map_ascii_keys() {
        assert_eq!(char_to_vk('a'), Some(0x41)); // VK_A
        assert_eq!(char_to_vk('z'), Some(0x5A)); // VK_Z
        assert_eq!(char_to_vk('A'), Some(0x41)); // 大小写同 VK，由 Shift 修饰键区分
        assert_eq!(char_to_vk('0'), Some(0x30)); // VK_0
        assert_eq!(char_to_vk('9'), Some(0x39)); // VK_9
        assert_eq!(char_to_vk('-'), Some(0xBD)); // VK_OEM_MINUS
        assert_eq!(char_to_vk(' '), Some(0x20)); // VK_SPACE
        assert_eq!(char_to_vk('/'), Some(0xBF)); // VK_OEM_2
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn char_to_vk_should_not_map_non_ascii() {
        assert_eq!(char_to_vk('中'), None);
        assert_eq!(char_to_vk('你'), None);
    }

    #[test]
    fn normalized_region_should_include_screen_offsets() {
        let region = NormalizedRegion {
            x: 0.25,
            y: 0.5,
            width: 0.4,
            height: 0.25,
        };
        let bounds = ScreenBounds {
            x: 100,
            y: 200,
            width: 800,
            height: 600,
        };

        let screen = normalized_region_to_screen(&region, &bounds);

        assert_eq!(screen.x, 300);
        assert_eq!(screen.y, 500);
        assert_eq!(screen.width, 320);
        assert_eq!(screen.height, 150);
    }

    #[test]
    fn default_operate_screenshot_path_should_be_named_with_timestamp() {
        let root = std::path::Path::new("C:/tmp/screenshots");
        let path = default_operate_screenshot_path(root);
        let file_name = std::path::Path::new(&path)
            .file_name()
            .expect("path should have a file name")
            .to_string_lossy()
            .to_string();
        assert!(file_name.starts_with("operate_"), "unexpected file name: {file_name}");
        assert!(file_name.ends_with(".webp"), "unexpected file name: {file_name}");
        let ms_part = file_name
            .trim_start_matches("operate_")
            .trim_end_matches(".webp");
        assert!(
            ms_part.parse::<u128>().is_ok(),
            "timestamp part should be numeric: {file_name}"
        );
        // 默认路径必须落在传入的会话截图根目录下（按会话建目录）。
        let parent = std::path::Path::new(&path)
            .parent()
            .expect("path should have a parent");
        let parent_norm = parent.to_string_lossy().replace('\\', "/");
        assert_eq!(
            parent_norm, "C:/tmp/screenshots",
            "default path must stay inside the per-conversation screenshots root"
        );
    }
}
