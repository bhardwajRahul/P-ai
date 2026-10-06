fn inflight_chat_key(
    agent_id: &str,
    conversation_id: Option<&str>,
) -> String {
    let agent_id = agent_id.trim();
    match conversation_id.map(str::trim).filter(|value| !value.is_empty()) {
        Some(conversation_id) if agent_id.is_empty() => conversation_id.to_string(),
        Some(conversation_id) => format!("{}::{}", agent_id, conversation_id),
        None => agent_id.to_string(),
    }
}

/// 登记一个会话轮次的打断句柄。
///
/// 若该会话上已有「停止意图」（停止命令早于本句柄登记到达），则消费该意图并返回 `true`，
/// 表示这个轮次应当立即中止、不要开始。停止意图不会过期，必须被消费。
fn register_inflight_chat_abort_handle(
    state: &AppState,
    chat_key: &str,
    handle: AbortHandle,
) -> Result<bool, String> {
    let mut inflight = state
        .inflight_chat_abort_handles
        .lock()
        .map_err(|_| "Failed to lock inflight chat abort handles".to_string())?;
    if matches!(
        inflight.get(chat_key),
        Some(InflightChatAbortEntry::StopRequested)
    ) {
        inflight.remove(chat_key);
        return Ok(true);
    }
    if let Some(previous) = inflight.insert(
        chat_key.to_string(),
        InflightChatAbortEntry::Running(handle),
    ) {
        if let InflightChatAbortEntry::Running(previous) = previous {
            previous.abort();
        }
    }
    Ok(false)
}

/// 对指定会话发起停止。
///
/// 已有运行中句柄则中止并移除；尚无句柄（轮次还没登记）或已有停止意图，则留下/刷新停止意图。
/// 两种情况都算「停止已受理」。
fn request_stop_inflight_chat(state: &AppState, chat_key: &str) -> Result<bool, String> {
    let mut inflight = state
        .inflight_chat_abort_handles
        .lock()
        .map_err(|_| "Failed to lock inflight chat abort handles".to_string())?;
    match inflight.remove(chat_key) {
        Some(InflightChatAbortEntry::Running(handle)) => {
            handle.abort();
            Ok(true)
        }
        _ => {
            inflight.insert(
                chat_key.to_string(),
                InflightChatAbortEntry::StopRequested,
            );
            Ok(true)
        }
    }
}

/// 消费并清除指定会话的停止意图；返回是否命中。
///
/// 用于「轮次已经确定被打断」的收尾点，避免意图残留并误伤后续轮次。
fn take_inflight_chat_stop_intent(state: &AppState, chat_key: &str) -> Result<bool, String> {
    let mut inflight = state
        .inflight_chat_abort_handles
        .lock()
        .map_err(|_| "Failed to lock inflight chat abort handles".to_string())?;
    match inflight.get(chat_key) {
        Some(InflightChatAbortEntry::StopRequested) => {
            inflight.remove(chat_key);
            Ok(true)
        }
        _ => Ok(false),
    }
}

/// 清除指定会话的停止意图（陈旧意图清理）；已登记的运行句柄不受影响。
fn clear_inflight_chat_stop_intent(state: &AppState, chat_key: &str) -> Result<(), String> {
    let mut inflight = state
        .inflight_chat_abort_handles
        .lock()
        .map_err(|_| "Failed to lock inflight chat abort handles".to_string())?;
    if matches!(
        inflight.get(chat_key),
        Some(InflightChatAbortEntry::StopRequested)
    ) {
        inflight.remove(chat_key);
    }
    Ok(())
}

/// 清除某个会话上所有残留的停止意图（按会话 id 匹配 key 后缀）。
///
/// 会话的批次已经收尾后，任何仍挂着的停止意图都失去了作用对象：它想打断的那一轮
/// 已经结束或根本没开始。此时必须清掉，否则会误伤该会话后面真正的新轮次。
fn clear_conversation_stop_intents(
    state: &AppState,
    conversation_id: &str,
) -> Result<usize, String> {
    let conversation_id = conversation_id.trim();
    if conversation_id.is_empty() {
        return Ok(0);
    }
    let mut inflight = state
        .inflight_chat_abort_handles
        .lock()
        .map_err(|_| "Failed to lock inflight chat abort handles".to_string())?;
    let suffix = format!("::{conversation_id}");
    let mut stale_keys: Vec<String> = Vec::new();
    for (key, entry) in inflight.iter() {
        if !matches!(entry, InflightChatAbortEntry::StopRequested) {
            continue;
        }
        if key.as_str() == conversation_id || key.ends_with(&suffix) {
            stale_keys.push(key.clone());
        }
    }
    let cleared = stale_keys.len();
    for key in stale_keys {
        inflight.remove(&key);
    }
    Ok(cleared)
}

/// 移除并中止指定会话正在运行的轮次句柄；命中返回 `true`。
/// 该 key 上若只有停止意图，则保留意图（它针对的是尚未登记的轮次），返回 `false`。
fn abort_running_inflight_chat(state: &AppState, chat_key: &str) -> Result<bool, String> {
    let mut inflight = state
        .inflight_chat_abort_handles
        .lock()
        .map_err(|_| "Failed to lock inflight chat abort handles".to_string())?;
    if matches!(
        inflight.get(chat_key),
        Some(InflightChatAbortEntry::Running(_))
    ) {
        if let Some(InflightChatAbortEntry::Running(handle)) = inflight.remove(chat_key) {
            handle.abort();
        }
        return Ok(true);
    }
    Ok(false)
}

fn register_inflight_tool_abort_handle(
    state: &AppState,
    chat_key: &str,
    handle: AbortHandle,
) -> Result<(), String> {
    let mut inflight = state
        .inflight_tool_abort_handles
        .lock()
        .map_err(|_| "Failed to lock inflight tool abort handles".to_string())?;
    if let Some(previous) = inflight.insert(chat_key.to_string(), handle) {
        previous.abort();
    }
    Ok(())
}

fn reset_inflight_completed_tool_history(state: &AppState, chat_key: &str) -> Result<(), String> {
    let mut inflight = state
        .inflight_completed_tool_history
        .lock()
        .map_err(|_| "Failed to lock inflight completed tool history".to_string())?;
    inflight.insert(chat_key.to_string(), Vec::new());
    Ok(())
}

fn replace_inflight_completed_tool_history(
    state: &AppState,
    chat_key: &str,
    events: &[Value],
) -> Result<(), String> {
    let mut inflight = state
        .inflight_completed_tool_history
        .lock()
        .map_err(|_| "Failed to lock inflight completed tool history".to_string())?;
    inflight.insert(chat_key.to_string(), events.to_vec());
    Ok(())
}

fn inflight_completed_tool_history(
    state: &AppState,
    chat_key: &str,
) -> Result<Vec<Value>, String> {
    let inflight = state
        .inflight_completed_tool_history
        .lock()
        .map_err(|_| "Failed to lock inflight completed tool history".to_string())?;
    Ok(inflight.get(chat_key).cloned().unwrap_or_default())
}

fn clear_inflight_completed_tool_history(state: &AppState, chat_key: &str) -> Result<(), String> {
    let mut inflight = state
        .inflight_completed_tool_history
        .lock()
        .map_err(|_| "Failed to lock inflight completed tool history".to_string())?;
    inflight.remove(chat_key);
    Ok(())
}

fn clear_inflight_tool_abort_handle(state: &AppState, chat_key: &str) -> Result<(), String> {
    let mut inflight = state
        .inflight_tool_abort_handles
        .lock()
        .map_err(|_| "Failed to lock inflight tool abort handles".to_string())?;
    inflight.remove(chat_key);
    Ok(())
}

fn abort_inflight_tool_abort_handle(state: &AppState, chat_key: &str) -> Result<bool, String> {
    let mut inflight = state
        .inflight_tool_abort_handles
        .lock()
        .map_err(|_| "Failed to lock inflight tool abort handles".to_string())?;
    if let Some(handle) = inflight.remove(chat_key) {
        handle.abort();
        Ok(true)
    } else {
        Ok(false)
    }
}

fn delegate_thread_chat_key(thread: &DelegateRuntimeThread) -> String {
    inflight_chat_key(
        &thread.conversation.agent_id,
        Some(&thread.conversation.id),
    )
}

fn abort_delegate_runtime_descendant_threads(
    state: &AppState,
    parent_chat_key: &str,
    children: Vec<DelegateRuntimeThread>,
) -> Result<usize, String> {
    let mut aborted_count = 0usize;
    for thread in children {
        let child_chat_key = delegate_thread_chat_key(&thread);
        let aborted_chat = abort_running_inflight_chat(state, &child_chat_key)?;
        let aborted_tool = abort_inflight_tool_abort_handle(state, &child_chat_key)?;
        if aborted_chat || aborted_tool {
            aborted_count += 1;
            runtime_log_info(format!(
                "[聊天] 已中止同步委托子会话: parent_session={}, child_session={}, delegate_id={}",
                parent_chat_key,
                child_chat_key,
                thread.delegate_id
            ));
        }
        aborted_count += abort_delegate_runtime_descendants_by_parent_session(state, &child_chat_key)?;
    }
    Ok(aborted_count)
}

fn abort_delegate_runtime_descendants_by_parent_session(
    state: &AppState,
    parent_chat_key: &str,
) -> Result<usize, String> {
    let children = delegate_runtime_thread_list(state)?
        .into_iter()
        .filter(|thread| thread.parent_chat_session_key.as_deref() == Some(parent_chat_key))
        .collect::<Vec<_>>();
    abort_delegate_runtime_descendant_threads(state, parent_chat_key, children)
}

fn abort_delegate_runtime_descendants_by_parent_context(
    state: &AppState,
    parent_chat_key: &str,
    root_conversation_id: Option<&str>,
) -> Result<usize, String> {
    let root_conversation_id = root_conversation_id
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let Some(root_conversation_id) = root_conversation_id else {
        return abort_delegate_runtime_descendants_by_parent_session(state, parent_chat_key);
    };

    let mut seen_delegate_ids = std::collections::HashSet::new();
    let children = delegate_runtime_thread_list(state)?
        .into_iter()
        .filter(|thread| {
            let exact_child = thread.parent_chat_session_key.as_deref() == Some(parent_chat_key);
            let same_root_sync_child = thread.root_conversation_id == root_conversation_id
                && thread.parent_chat_session_key.is_some();
            exact_child || same_root_sync_child
        })
        .filter(|thread| seen_delegate_ids.insert(thread.delegate_id.clone()))
        .collect::<Vec<_>>();
    abort_delegate_runtime_descendant_threads(state, parent_chat_key, children)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModelReplyContentState {
    Visible,
    ReasoningOnly,
    Empty,
}

fn model_reply_content_state(reply: &ModelReply) -> ModelReplyContentState {
    if !reply.assistant_text.trim().is_empty()
        || !reply.final_response_text.trim().is_empty()
        || reply.assistant_provider_meta.is_some()
        || !reply.tool_history_events.is_empty()
        || reply.suppress_assistant_message
    {
        return ModelReplyContentState::Visible;
    }
    if !reply.activity_reasoning_text.trim().is_empty() {
        return ModelReplyContentState::ReasoningOnly;
    }
    ModelReplyContentState::Empty
}

fn effective_prompt_tokens_from_provider(
    estimated_prompt_tokens: u64,
    trusted_input_tokens: Option<u64>,
) -> (u64, &'static str) {
    let estimated = estimated_prompt_tokens.max(1);
    let Some(provider) = trusted_input_tokens.filter(|value| *value > 0) else {
        return (estimated_prompt_tokens, "estimate_no_provider");
    };
    let gap = provider.abs_diff(estimated) as f64 / estimated as f64;
    if gap > 0.5 {
        return (provider.max(estimated_prompt_tokens), "max_large_gap");
    }
    (provider, "provider")
}
