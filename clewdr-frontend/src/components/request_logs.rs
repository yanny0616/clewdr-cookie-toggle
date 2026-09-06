use crate::api;
use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

#[component]
pub fn RequestLogsTab() -> impl IntoView {
    let logs = RwSignal::new(Vec::<api::ApiRequestLog>::new());
    let loading = RwSignal::new(false);
    let error = RwSignal::new(Option::<String>::None);
    let filter = RwSignal::new("all".to_string());
    let search = RwSignal::new(String::new());
    let confirm_clear = RwSignal::new(false);
    let now = RwSignal::new(js_sys::Date::now() as i64);
    let load = move || {
        if loading.get_untracked() {
            return;
        }
        loading.set(true);
        spawn_local(async move {
            match api::get_request_logs(500).await {
                Ok(items) => {
                    let _ = logs.try_set(items);
                    let _ = error.try_set(None);
                }
                Err(err) => {
                    let _ = error.try_set(Some(err));
                }
            }
            let _ = loading.try_set(false);
        });
    };
    load();
    let timer = gloo_timers::callback::Interval::new(1000, move || {
        now.set(js_sys::Date::now() as i64);
    });
    let refresh = gloo_timers::callback::Interval::new(5000, load);
    let timers = StoredValue::new_local((timer, refresh));
    on_cleanup(move || {
        timers.dispose();
    });
    let filtered = Memo::new(move |_| {
        let query = search.get().to_lowercase();
        logs.get()
            .into_iter()
            .filter(|l| {
                matches_filter(&l.status, &filter.get())
                    && format!(
                        "{} {} {} {}",
                        l.id,
                        l.model,
                        l.error_code.as_deref().unwrap_or(""),
                        l.error.as_deref().unwrap_or("")
                    )
                    .to_lowercase()
                    .contains(&query)
            })
            .collect::<Vec<_>>()
    });
    view! {
        <section class="stack request-log-panel">
            <div class="row-btw log-heading">
                <div><h3>"请求记录"</h3><p class="text-dim text-sm">"每 5 秒刷新 · 最近 500 条 · 点击请求查看详情"</p></div>
                <div class="row-sm"><button class="btn btn-ghost" disabled=move || loading.get() on:click=move |_| load()>
                    {move || if loading.get() { "刷新中…" } else { "刷新" }}
                </button>
                <button class="btn btn-ghost" on:click=move |_| confirm_clear.set(true)>"清空记录"</button></div>
            </div>
            <Show when=move || confirm_clear.get()>
                <div class="alert row-btw"><span>"清空所有请求记录？此操作不可恢复。"</span><div class="row-sm">
                    <button class="btn btn-ghost" on:click=move |_| confirm_clear.set(false)>"取消"</button>
                    <button class="btn btn-danger" disabled=move || loading.get() on:click=move |_| {
                        loading.set(true);
                        spawn_local(async move {
                            match api::clear_request_logs().await {
                                Ok(()) => { let _ = logs.try_set(Vec::new()); let _ = confirm_clear.try_set(false); }
                                Err(err) => { let _ = error.try_set(Some(err)); }
                            }
                            let _ = loading.try_set(false);
                        });
                    }>"确认清空"</button>
                </div></div>
            </Show>
            <div class="log-overview">
                {[( "all", "全部"), ("pending", "处理中"), ("success", "成功"), ("failed", "异常")].into_iter().map(|(key, label)| view! {
                    <button class=move || if filter.get() == key { "log-stat active" } else { "log-stat" }
                        aria-pressed=move || (filter.get() == key).to_string()
                        on:click=move |_| filter.set(key.into())>
                        <span>{label}</span><strong>{move || logs.get().iter().filter(|l| matches_filter(&l.status, key)).count()}</strong>
                    </button>
                }).collect::<Vec<_>>()}
            </div>
            <input class="input log-search" type="search" placeholder="搜索模型、请求 ID 或错误信息…"
                prop:value=move || search.get() on:input=move |ev| search.set(event_target_value(&ev)) />
            <Show when=move || error.get().is_some()>
                <div class="alert alert-error">{move || error.get().unwrap_or_default()}</div>
            </Show>
            <div class="log-list">
                <Show when=move || filtered.get().is_empty()><div class="log-empty">"没有符合条件的请求"</div></Show>
                <For each=move || filtered.get() key=|l| serde_json::to_string(l).unwrap_or_default()
                    children=move |log| view! { <RequestLogRow log now /> } />
            </div>
            <p class="text-dim text-sm">"取消表示下游断开；已中断表示服务启动时发现历史记录未收尾。历史记录缺失的原因不会推测填充。"</p>
        </section>
    }
}

fn matches_filter(status: &str, filter: &str) -> bool {
    match filter {
        "all" => true,
        "failed" => !matches!(status, "pending" | "success"),
        other => status == other,
    }
}

#[component]
fn RequestLogRow(log: api::ApiRequestLog, now: RwSignal<i64>) -> impl IntoView {
    let (status_class, label) = match log.status.as_str() {
        "success" => ("log-status log-status-success", "成功"),
        "pending" => ("log-status log-status-pending", "处理中"),
        "timeout" => ("log-status log-status-error", "超时"),
        "cancelled" => ("log-status log-status-muted", "已取消"),
        "interrupted" => ("log-status log-status-muted", "已中断"),
        _ => ("log-status log-status-error", "失败"),
    };
    let pending = log.status == "pending";
    let timestamp = log.timestamp_ms;
    let duration = log.duration_ms;
    let error = log.error.clone().unwrap_or_default();
    let summary = error.chars().take(180).collect::<String>();
    view! {
        <details class="log-entry">
            <summary>
                <div class="log-entry-main">
                    <span class=status_class>{label}</span>
                    <span class="log-entry-model">{log.model}</span>
                    <span class="log-entry-duration">{move || {
                        let ms = if pending { Some(now.get().saturating_sub(timestamp)) } else { duration };
                        ms.map(|v| format!("{:.1}s", v.max(0) as f64 / 1000.0)).unwrap_or_else(|| "—".into())
                    }}</span>
                </div>
                <div class="log-entry-meta">
                    <span>{format_time(timestamp)}</span><span>{format!("#{}", log.id)}</span>
                    <span>{if log.stream { "流式" } else { "非流式" }}</span>
                    <span>{format!("{} · {}", log.provider, log.api_format)}</span>
                    <span>{format!("输入 {} / 输出 {}", format_num(log.input_tokens), format_num(log.output_tokens))}</span>
                </div>
                {(!error.is_empty()).then(|| view! { <p class="log-entry-error">{summary}</p> })}
            </summary>
            <div class="log-entry-details">
                <dl class="log-metrics">
                    <div><dt>"估算上下文"</dt><dd>{format_num(Some(log.estimated_context_tokens as u64))}</dd></div>
                    <div><dt>"输入 tokens"</dt><dd>{format_num(log.input_tokens)}</dd></div>
                    <div><dt>"输出 tokens"</dt><dd>{format_num(log.output_tokens)}</dd></div>
                    <div><dt>"缓存写入"</dt><dd>{format_num(log.cache_creation_input_tokens)}</dd></div>
                    <div><dt>"缓存读取"</dt><dd>{format_num(log.cache_read_input_tokens)}</dd></div>
                    <div><dt>"缓存断点"</dt><dd>{log.cache_control_breakpoints}</dd></div>
                    <div><dt>"消息 / 工具"</dt><dd>{format!("{} / {}", log.message_count, log.tool_count)}</dd></div>
                    <div><dt>"错误代码"</dt><dd>{log.error_code.unwrap_or_else(|| "—".into())}</dd></div>
                </dl>
                {(!error.is_empty()).then(|| view! { <pre class="log-error-detail">{error}</pre> })}
            </div>
        </details>
    }
}

fn format_num(value: Option<u64>) -> String {
    value.map(|v| v.to_string()).unwrap_or_else(|| "—".into())
}

fn format_time(timestamp_ms: i64) -> String {
    let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(timestamp_ms as f64));
    format!(
        "{:02}-{:02} {:02}:{:02}:{:02}",
        date.get_month() + 1,
        date.get_date(),
        date.get_hours(),
        date.get_minutes(),
        date.get_seconds()
    )
}
