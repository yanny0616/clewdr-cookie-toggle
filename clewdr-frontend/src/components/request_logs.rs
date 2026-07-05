use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::api;

#[component]
pub fn RequestLogsTab() -> impl IntoView {
    let logs = RwSignal::new(Vec::<api::ApiRequestLog>::new());
    let loading = RwSignal::new(false);
    let error = RwSignal::new(Option::<String>::None);

    let load = move || {
        loading.set(true);
        error.set(None);
        spawn_local(async move {
            match api::get_request_logs(100).await {
                Ok(items) => logs.set(items),
                Err(err) => error.set(Some(err)),
            }
            loading.set(false);
        });
    };

    load();

    let clear = move |_| {
        loading.set(true);
        error.set(None);
        spawn_local(async move {
            match api::clear_request_logs().await {
                Ok(()) => logs.set(Vec::new()),
                Err(err) => error.set(Some(err)),
            }
            loading.set(false);
        });
    };

    view! {
        <div class="stack">
            <div class="row-btw" style="align-items:flex-start; gap:1rem; flex-wrap:wrap">
                <div>
                    <h3>"API 请求日志"</h3>
                    <p class="text-dim text-sm">
                        "只记录请求摘要和 token/cache 统计，不保存完整上下文或角色卡。Claude Web 通常没有官方缓存读写字段。"
                    </p>
                </div>
                <div class="row-sm">
                    <button class="btn btn-secondary" disabled=move || loading.get() on:click=move |_| load()>
                        {move || if loading.get() { "刷新中..." } else { "刷新" }}
                    </button>
                    <button class="btn btn-danger" disabled=move || loading.get() on:click=clear>
                        "清空"
                    </button>
                </div>
            </div>

            <Show when=move || error.get().is_some()>
                <div class="alert alert-error">{move || error.get().unwrap_or_default()}</div>
            </Show>

            <Show
                when=move || !logs.get().is_empty()
                fallback=move || view! { <p class="text-dim">"还没有 API 请求记录。"</p> }
            >
                <div style="overflow-x:auto">
                    <table class="usage-table" style="min-width:1040px">
                        <thead>
                            <tr>
                                <th>"ID"</th>
                                <th>"时间"</th>
                                <th>"状态"</th>
                                <th>"来源"</th>
                                <th>"格式"</th>
                                <th>"模式"</th>
                                <th>"消息/工具"</th>
                                <th>"模型"</th>
                                <th>"上下文"</th>
                                <th>"输入"</th>
                                <th>"输出"</th>
                                <th>"Cache 写"</th>
                                <th>"Cache 读"</th>
                                <th>"断点"</th>
                                <th>"耗时"</th>
                                <th>"错误"</th>
                            </tr>
                        </thead>
                        <tbody>
                            {move || logs.get().into_iter().map(|log| view! { <RequestLogRow log /> }).collect::<Vec<_>>()}
                        </tbody>
                    </table>
                </div>
            </Show>
        </div>
    }
}

#[component]
fn RequestLogRow(log: api::ApiRequestLog) -> impl IntoView {
    let status_class = match log.status.as_str() {
        "success" => "badge valid",
        "error" => "badge exhausted",
        _ => "badge unknown",
    };
    view! {
        <tr>
            <td>{log.id}</td>
            <td>{format_time(log.timestamp_ms)}</td>
            <td><span class=status_class>{log.status}</span></td>
            <td>{log.provider}</td>
            <td>{log.api_format}</td>
            <td>{if log.stream { "stream" } else { "normal" }}</td>
            <td>{format!("{} / {}", log.message_count, log.tool_count)}</td>
            <td style="max-width:220px; white-space:normal">{log.model}</td>
            <td>{format_num(Some(log.estimated_context_tokens as u64))}</td>
            <td>{format_num(log.input_tokens)}</td>
            <td>{format_num(log.output_tokens)}</td>
            <td>{format_num(log.cache_creation_input_tokens)}</td>
            <td>{format_num(log.cache_read_input_tokens)}</td>
            <td>{log.cache_control_breakpoints}</td>
            <td>{log.duration_ms.map(|v| format!("{v}ms")).unwrap_or_else(|| "-".into())}</td>
            <td style="max-width:260px; white-space:normal">{log.error.unwrap_or_default()}</td>
        </tr>
    }
}

fn format_num(value: Option<u64>) -> String {
    value.map(|v| v.to_string()).unwrap_or_else(|| "-".into())
}

fn format_time(timestamp_ms: i64) -> String {
    let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(timestamp_ms as f64));
    format!(
        "{:02}:{:02}:{:02}",
        date.get_hours(),
        date.get_minutes(),
        date.get_seconds()
    )
}
