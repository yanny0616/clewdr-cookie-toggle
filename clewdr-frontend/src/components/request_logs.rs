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
                    <button class="btn btn-ghost" disabled=move || loading.get() on:click=move |_| load()>
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
                fallback=move || view! {
                    <div class="log-empty">"还没有 API 请求记录。"</div>
                }
            >
                <div class="log-table-wrap">
                    <table class="log-table">
                        <thead>
                            <tr>
                                <th>"ID"</th>
                                <th>"时间"</th>
                                <th>"状态"</th>
                                <th>"错误代码"</th>
                                <th>"错误信息"</th>
                                <th>"来源"</th>
                                <th>"格式"</th>
                                <th>"模式"</th>
                                <th>"模型"</th>
                                <th>"消息/工具"</th>
                                <th>"上下文"</th>
                                <th>"输入"</th>
                                <th>"输出"</th>
                                <th>"Cache 写"</th>
                                <th>"Cache 读"</th>
                                <th>"断点"</th>
                                <th>"耗时"</th>
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
        "success" => "log-status log-status-success",
        "error" => "log-status log-status-error",
        _ => "log-status log-status-pending",
    };
    let error_title = log.error.clone().unwrap_or_default();
    let error_text = empty_dash(log.error);
    let model_title = log.model.clone();
    let model_text = log.model;
    view! {
        <tr>
            <td class="log-num">{log.id}</td>
            <td class="log-time">{format_time(log.timestamp_ms)}</td>
            <td><span class=status_class>{log.status}</span></td>
            <td class="log-code">{log.error_code.unwrap_or_else(|| "-".into())}</td>
            <td class="log-error" title=error_title>
                {error_text}
            </td>
            <td class="log-source">{log.provider}</td>
            <td>{log.api_format}</td>
            <td>
                <span class=if log.stream { "log-mode log-mode-stream" } else { "log-mode" }>
                    {if log.stream { "stream" } else { "normal" }}
                </span>
            </td>
            <td class="log-model" title=model_title>{model_text}</td>
            <td class="log-num">{format!("{} / {}", log.message_count, log.tool_count)}</td>
            <td class="log-num">{format_num(Some(log.estimated_context_tokens as u64))}</td>
            <td class="log-num">{format_num(log.input_tokens)}</td>
            <td class="log-num">{format_num(log.output_tokens)}</td>
            <td class="log-num">{format_num(log.cache_creation_input_tokens)}</td>
            <td class="log-num">{format_num(log.cache_read_input_tokens)}</td>
            <td class="log-num">{log.cache_control_breakpoints}</td>
            <td class="log-num">{log.duration_ms.map(|v| format!("{v}ms")).unwrap_or_else(|| "-".into())}</td>
        </tr>
    }
}

fn format_num(value: Option<u64>) -> String {
    value.map(|v| v.to_string()).unwrap_or_else(|| "-".into())
}

fn empty_dash(value: Option<String>) -> String {
    value
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "-".into())
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
