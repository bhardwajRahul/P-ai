#[tauri::command]
async fn generate_image(
    request: ImageGenerationRequest,
    state: State<'_, AppState>,
) -> Result<ImageGenerationResult, String> {
    generate_images(state.inner(), request).await
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImageToVideoTestRequest {
    image: String,
    #[serde(default)]
    model_id: Option<String>,
    #[serde(default)]
    prompt: String,
    #[serde(default = "default_image_to_video_duration")]
    duration: u32,
    #[serde(default = "default_image_to_video_resolution")]
    resolution: String,
}

fn default_image_to_video_duration() -> u32 {
    6
}

fn default_image_to_video_resolution() -> String {
    "480p".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImageToVideoTestResult {
    path: String,
    absolute_path: String,
}

fn resolve_assistant_space_absolute_path(state: &AppState, display: &str) -> Result<String, String> {
    let relative = assistant_space_relative_image_path(display)?
        .ok_or_else(|| format!("视频路径不是 Assistant Space 路径：{display}"))?;
    let workspace_root = configured_workspace_root_path(state)
        .unwrap_or_else(|_| state.llm_workspace_path.clone());
    Ok(workspace_root
        .join(relative)
        .to_string_lossy()
        .replace('\\', "/"))
}

async fn test_image_to_video_inner(
    state: &AppState,
    request: ImageToVideoTestRequest,
) -> Result<ImageToVideoTestResult, String> {
    let first_frame = load_image_edit_reference(state, &request.image, "首帧").await?;
    let path = generate_grok_image_to_video(
        state,
        GrokImageToVideoRequest {
            image_url: image_edit_data_url(&first_frame),
            model_id: request.model_id,
            prompt: request.prompt,
            duration: request.duration,
            resolution: request.resolution,
        },
    )
    .await?;
    let absolute_path = resolve_assistant_space_absolute_path(state, &path)?;
    Ok(ImageToVideoTestResult { path, absolute_path })
}

#[tauri::command]
async fn test_image_to_video(
    request: ImageToVideoTestRequest,
    state: State<'_, AppState>,
) -> Result<ImageToVideoTestResult, String> {
    test_image_to_video_inner(state.inner(), request).await
}
