use std::collections::BTreeMap;

use gloo_timers::callback::{Interval, Timeout};
use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;

use crate::{
    api::{self, ApiRequestLog},
    i18n::{I18n, use_i18n},
    storage, utils,
};

const LOG_LIMIT: usize = 500;
const REFRESH_MS: u32 = 5000;
const PAGE_SIZE: usize = 50;
const AUTO_REFRESH_KEY: &str = "logsAutoRefresh";
const FAILURE_KINDS: [&str; 4] = ["error", "timeout", "cancelled", "interrupted"];

#[derive(Clone, Copy, PartialEq, Eq)]
enum StatusFilter {
    All,
    Pending,
    Success,
    Failed,
}

impl StatusFilter {
    const ALL: [Self; 4] = [Self::All, Self::Pending, Self::Success, Self::Failed];

    fn key(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Pending => "pending",
            Self::Success => "success",
            Self::Failed => "failed",
        }
    }

    fn matches(self, status: &str) -> bool {
        match self {
            Self::All => true,
            Self::Pending => status == "pending",
            Self::Success => status == "success",
            Self::Failed => !matches!(status, "pending" | "success"),
        }
    }
}

#[derive(Clone, PartialEq, Default)]
struct Summary {
    total: usize,
    finished: usize,
    success: usize,
    duration_sum: i64,
    duration_count: usize,
    input: u64,
    output: u64,
    cache_read: u64,
}

impl Summary {
    fn from_logs(logs: &[ApiRequestLog]) -> Self {
        let mut s = Self {
            total: logs.len(),
            ..Default::default()
        };
        for log in logs {
            if log.status != "pending" {
                s.finished += 1;
            }
            if log.status == "success" {
                s.success += 1;
                if let Some(ms) = log.duration_ms {
                    s.duration_sum += ms.max(0);
                    s.duration_count += 1;
                }
            }
            if let Some(input) = total_input(log) {
                s.input += input;
                s.cache_read += log.cache_read_input_tokens.unwrap_or(0);
            }
            s.output += log.output_tokens.unwrap_or(0);
        }
        s
    }

    fn success_rate(&self) -> String {
        fmt_pct(self.success as u64, self.finished as u64)
    }

    fn avg_duration(&self) -> String {
        if self.duration_count == 0 {
            "—".into()
        } else {
            fmt_duration(self.duration_sum / self.duration_count as i64)
        }
    }

    fn cache_hit(&self) -> String {
        fmt_pct(self.cache_read, self.input)
    }
}

#[component]
pub fn RequestLogsTab() -> impl IntoView {
    let i18n = use_i18n();
    let logs = RwSignal::new(Vec::<ApiRequestLog>::new());
    let loading = RwSignal::new(false);
    let error = RwSignal::new(Option::<String>::None);
    let filter = RwSignal::new(StatusFilter::All);
    let failure_kind = RwSignal::new(Option::<&'static str>::None);
    let model_filter = RwSignal::new(Option::<String>::None);
    let search = RwSignal::new(String::new());
    let shown = RwSignal::new(PAGE_SIZE);
    let confirm_clear = RwSignal::new(false);
    let auto_refresh = RwSignal::new(storage::get(AUTO_REFRESH_KEY).as_deref() != Some("off"));
    let last_updated = RwSignal::new(Option::<i64>::None);
    let now = RwSignal::new(js_sys::Date::now() as i64);

    let load = move || {
        if loading.get_untracked() {
            return;
        }
        loading.set(true);
        spawn_local(async move {
            match api::get_request_logs(LOG_LIMIT).await {
                Ok(items) => {
                    let _ = logs.try_set(items);
                    let _ = error.try_set(None);
                    let _ = last_updated.try_set(Some(js_sys::Date::now() as i64));
                }
                Err(err) => {
                    let _ = error.try_set(Some(err));
                }
            }
            let _ = loading.try_set(false);
        });
    };
    load();

    let clock = Interval::new(1000, move || now.set(js_sys::Date::now() as i64));
    let refresh = Interval::new(REFRESH_MS, move || {
        // Skip polling while the page is off-screen or the user paused it.
        if auto_refresh.get_untracked() && !document_hidden() {
            load();
        }
    });
    let timers = StoredValue::new_local((clock, refresh));
    on_cleanup(move || timers.dispose());

    let counts = Memo::new(move |_| {
        logs.with(|all| {
            StatusFilter::ALL.map(|f| all.iter().filter(|l| f.matches(&l.status)).count())
        })
    });
    let failure_counts = Memo::new(move |_| {
        logs.with(|all| FAILURE_KINDS.map(|kind| (kind, all.iter().filter(|l| l.status == kind).count())))
    });
    let models = Memo::new(move |_| {
        let mut map = BTreeMap::<String, usize>::new();
        logs.with(|all| {
            for l in all {
                *map.entry(l.model.clone()).or_default() += 1;
            }
        });
        let mut list = map.into_iter().collect::<Vec<_>>();
        list.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        list
    });
    let filtered = Memo::new(move |_| {
        let status = filter.get();
        let kind = failure_kind.get();
        let model = model_filter.get();
        let query = search.get().trim().to_lowercase();
        logs.with(|all| {
            all.iter()
                .filter(|l| {
                    status.matches(&l.status)
                        && (status != StatusFilter::Failed || kind.is_none_or(|k| l.status == k))
                        && model.as_deref().is_none_or(|m| l.model == m)
                        && (query.is_empty() || haystack(l).contains(&query))
                })
                .cloned()
                .collect::<Vec<_>>()
        })
    });
    let summary = Memo::new(move |_| filtered.with(|list| Summary::from_logs(list)));
    let visible = Memo::new(move |_| {
        filtered.with(|list| list.iter().take(shown.get()).cloned().collect::<Vec<_>>())
    });

    let reset_page = move || shown.set(PAGE_SIZE);
    let clear_logs = move |_| {
        loading.set(true);
        spawn_local(async move {
            match api::clear_request_logs().await {
                Ok(()) => {
                    let _ = logs.try_set(Vec::new());
                    let _ = confirm_clear.try_set(false);
                }
                Err(err) => {
                    let _ = error.try_set(Some(err));
                }
            }
            let _ = loading.try_set(false);
        });
    };

    view! {
        <section class="stack request-log-panel">
            <div class="row-btw log-heading">
                <div>
                    <h3>{move || i18n.t("logs.title")}</h3>
                    <p class="text-dim text-sm">
                        {move || i18n.tf("logs.subtitle", &[("interval", &(REFRESH_MS / 1000).to_string()), ("limit", &LOG_LIMIT.to_string())])}
                    </p>
                </div>
                <div class="log-heading-actions">
                    <label class="log-switch" title=move || i18n.t("logs.autoRefreshHint")>
                        <input type="checkbox" prop:checked=move || auto_refresh.get()
                            on:change=move |ev| {
                                let on = event_target_checked(&ev);
                                auto_refresh.set(on);
                                storage::set(AUTO_REFRESH_KEY, if on { "on" } else { "off" });
                            } />
                        <span class=move || if auto_refresh.get() { "log-live on" } else { "log-live" }>
                            <i class="log-live-dot"></i>
                            {move || i18n.t("logs.autoRefresh")}
                        </span>
                    </label>
                    <button class="btn btn-ghost btn-sm" disabled=move || loading.get() on:click=move |_| load()>
                        {move || if loading.get() { i18n.t("logs.refreshing") } else { i18n.t("logs.refresh") }}
                    </button>
                    <button class="btn btn-ghost btn-sm" on:click=move |_| confirm_clear.set(true)>
                        {move || i18n.t("logs.clear")}
                    </button>
                </div>
            </div>

            <Show when=move || confirm_clear.get()>
                <div class="alert alert-warn row-btw">
                    <span>{move || i18n.t("logs.clearConfirm")}</span>
                    <div class="row-sm">
                        <button class="btn btn-ghost btn-sm" on:click=move |_| confirm_clear.set(false)>{move || i18n.t("logs.cancel")}</button>
                        <button class="btn btn-danger btn-sm" disabled=move || loading.get() on:click=clear_logs>{move || i18n.t("logs.confirmClear")}</button>
                    </div>
                </div>
            </Show>

            <div class="log-overview">
                {StatusFilter::ALL.into_iter().enumerate().map(|(i, f)| view! {
                    <button
                        class=move || format!("log-stat log-stat-{}{}", f.key(), if filter.get() == f { " active" } else { "" })
                        aria-pressed=move || (filter.get() == f).to_string()
                        on:click=move |_| { filter.set(f); failure_kind.set(None); reset_page(); }
                    >
                        <span class="log-stat-label">{move || i18n.t(&format!("logs.filter.{}", f.key()))}</span>
                        <strong>{move || counts.get()[i]}</strong>
                    </button>
                }).collect::<Vec<_>>()}
            </div>

            <div class="log-summary">
                <span><b>{move || summary.get().total}</b>{move || i18n.t("logs.summary.count")}</span>
                <span>{move || i18n.t("logs.summary.successRate")}<b>{move || summary.get().success_rate()}</b></span>
                <span>{move || i18n.t("logs.summary.avgDuration")}<b>{move || summary.get().avg_duration()}</b></span>
                <span>{move || i18n.t("logs.summary.input")}<b title=move || fmt_int(summary.get().input)>{move || fmt_compact(summary.get().input)}</b></span>
                <span>{move || i18n.t("logs.summary.output")}<b title=move || fmt_int(summary.get().output)>{move || fmt_compact(summary.get().output)}</b></span>
                <span>{move || i18n.t("logs.summary.cacheHit")}<b>{move || summary.get().cache_hit()}</b></span>
            </div>

            <div class="log-toolbar">
                <input class="input input-sm log-search" type="search"
                    placeholder=move || i18n.t("logs.searchPlaceholder")
                    prop:value=move || search.get()
                    on:input=move |ev| { search.set(event_target_value(&ev)); reset_page(); } />
                <button class="btn btn-ghost btn-sm" on:click=move |_| set_all_open(true)>{move || i18n.t("logs.expandAll")}</button>
                <button class="btn btn-ghost btn-sm" on:click=move |_| set_all_open(false)>{move || i18n.t("logs.collapseAll")}</button>
            </div>

            <Show when=move || { models.get().len() > 1 }>
                <div class="log-chips">
                    <span class="log-chips-label">{move || i18n.t("logs.filter.models")}</span>
                    <button class=move || if model_filter.get().is_none() { "log-chip active" } else { "log-chip" }
                        on:click=move |_| { model_filter.set(None); reset_page(); }>
                        {move || i18n.t("logs.filter.allModels")}
                    </button>
                    <For each=move || models.get() key=|(m, n)| format!("{m}:{n}") children=move |(model, count)| {
                        let selected = model.clone();
                        let chosen = model.clone();
                        view! {
                            <button class=move || if model_filter.get().as_deref() == Some(selected.as_str()) { "log-chip active" } else { "log-chip" }
                                on:click=move |_| { model_filter.set(Some(chosen.clone())); reset_page(); }>
                                {model}<span class="log-chip-count">{count}</span>
                            </button>
                        }
                    } />
                </div>
            </Show>

            <Show when=move || filter.get() == StatusFilter::Failed>
                <div class="log-chips">
                    <span class="log-chips-label">{move || i18n.t("logs.filter.failures")}</span>
                    <button class=move || if failure_kind.get().is_none() { "log-chip active" } else { "log-chip" }
                        on:click=move |_| { failure_kind.set(None); reset_page(); }>
                        {move || i18n.t("logs.filter.allFailed")}
                    </button>
                    {FAILURE_KINDS.into_iter().enumerate().map(|(i, kind)| view! {
                        <button class=move || if failure_kind.get() == Some(kind) { "log-chip active" } else { "log-chip" }
                            on:click=move |_| { failure_kind.set(Some(kind)); reset_page(); }>
                            {move || i18n.t(&format!("logs.filter.{kind}"))}
                            <span class="log-chip-count">{move || failure_counts.get()[i].1}</span>
                        </button>
                    }).collect::<Vec<_>>()}
                </div>
            </Show>

            <Show when=move || error.get().is_some()>
                <div class="alert alert-error">{move || error.get().unwrap_or_default()}</div>
            </Show>

            <div class="log-list">
                <Show when=move || filtered.with(|l| l.is_empty())>
                    <div class="log-empty">{move || i18n.t("logs.empty")}</div>
                </Show>
                <For each=move || visible.get() key=|l| l.id children=move |log| {
                    let id = log.id;
                    // Keyed by id so an open row survives refreshes; the memo keeps its data current.
                    let entry = Memo::new(move |_| {
                        logs.with(|all| all.iter().find(|l| l.id == id).cloned())
                            .unwrap_or_else(|| log.clone())
                    });
                    view! { <RequestLogRow entry now /> }
                } />
                <Show when=move || { filtered.with(|l| l.len() > shown.get()) }>
                    <div class="log-more">
                        <span>{move || i18n.tf("logs.showing", &[("shown", &visible.with(|v| v.len()).to_string()), ("total", &filtered.with(|f| f.len()).to_string())])}</span>
                        <button class="btn btn-ghost btn-sm" on:click=move |_| shown.update(|s| *s += PAGE_SIZE)>
                            {move || i18n.tf("logs.loadMore", &[("count", &PAGE_SIZE.to_string())])}
                        </button>
                        <button class="btn btn-ghost btn-sm" on:click=move |_| shown.set(usize::MAX)>{move || i18n.t("logs.showAll")}</button>
                    </div>
                </Show>
            </div>

            <div class="row-btw log-footer">
                <p class="log-footnote">{move || i18n.t("logs.footnote")}</p>
                <span class="log-updated">{move || match last_updated.get() {
                    Some(ts) => i18n.tf("logs.lastUpdated", &[("time", &fmt_clock(ts, now.get()))]),
                    None => String::new(),
                }}</span>
            </div>
        </section>
    }
}

#[component]
fn RequestLogRow(entry: Memo<ApiRequestLog>, now: RwSignal<i64>) -> impl IntoView {
    let i18n = use_i18n();
    let toast = expect_context::<RwSignal<Option<(String, bool)>>>();
    let kind = Memo::new(move |_| status_kind(&entry.get().status));
    let copy_json = move |_| {
        if let Ok(json) = serde_json::to_string_pretty(&entry.get_untracked()) {
            utils::copy_to_clipboard(json);
            toast.set(Some((i18n.t("logs.copied"), true)));
            Timeout::new(2000, move || toast.set(None)).forget();
        }
    };

    view! {
        <details class="log-entry" data-status=move || kind.get()>
            <summary>
                {move || {
                    let log = entry.get();
                    let kind = status_kind(&log.status);
                    let ts = log.timestamp_ms;
                    let pending = log.status == "pending";
                    let duration = log.duration_ms;
                    let input = total_input(&log);
                    let cache_pct = input
                        .filter(|total| *total > 0)
                        .zip(log.cache_read_input_tokens)
                        .map(|(total, read)| fmt_pct(read, total));
                    let error_line = log
                        .error
                        .as_deref()
                        .and_then(|e| e.lines().find(|l| !l.trim().is_empty()))
                        .map(|l| l.chars().take(240).collect::<String>());
                    view! {
                        <div class="log-entry-main">
                            <span class=format!("log-status log-status-{kind}")>
                                <i class="log-dot"></i>{i18n.t(&format!("logs.status.{kind}"))}
                            </span>
                            <span class="log-entry-model" title=log.model.clone()>{log.model.clone()}</span>
                            <span class=if log.stream { "log-tag log-tag-stream" } else { "log-tag" }>
                                {if log.stream { i18n.t("logs.stream") } else { i18n.t("logs.nonStream") }}
                            </span>
                            <span class="log-entry-duration">{move || {
                                let ms = if pending { Some(now.get().saturating_sub(ts)) } else { duration };
                                ms.map(fmt_duration).unwrap_or_else(|| "—".into())
                            }}</span>
                            <span class="log-entry-time" title=fmt_full(ts)>{move || {
                                let n = now.get();
                                match fmt_relative(i18n, ts, n) {
                                    Some(rel) => format!("{} · {}", fmt_clock(ts, n), rel),
                                    None => fmt_clock(ts, n),
                                }
                            }}</span>
                            <span class="log-chevron" aria-hidden="true"></span>
                        </div>
                        <div class="log-entry-meta">
                            <span class="mono">{format!("#{}", log.id)}</span>
                            <span>{format!("{} · {}", log.provider, log.api_format)}</span>
                            <span>{i18n.tf("logs.tokens", &[("input", &fmt_opt(input)), ("output", &fmt_opt(log.output_tokens))])}</span>
                            {cache_pct.map(|pct| view! { <span>{i18n.tf("logs.cacheHit", &[("pct", &pct)])}</span> })}
                            {log.error_code.clone().map(|code| view! { <span class="log-code">{code}</span> })}
                        </div>
                        {error_line.map(|line| view! { <p class="log-entry-error">{line}</p> })}
                    }
                }}
            </summary>
            <div class="log-entry-details">
                {move || {
                    let log = entry.get();
                    let input = total_input(&log);
                    let cache_pct = input
                        .filter(|total| *total > 0)
                        .zip(log.cache_read_input_tokens)
                        .map(|(total, read)| format!(" · {}", fmt_pct(read, total)))
                        .unwrap_or_default();
                    let source = format!(
                        "{} · {} · {}",
                        log.provider,
                        log.api_format,
                        if log.stream { i18n.t("logs.stream") } else { i18n.t("logs.nonStream") }
                    );
                    let error = log.error.clone().unwrap_or_default();
                    view! {
                        <dl class="log-metrics">
                            <div><dt>{i18n.t("logs.metrics.started")}</dt><dd>{fmt_full(log.timestamp_ms)}</dd></div>
                            <div><dt>{i18n.t("logs.metrics.duration")}</dt><dd>{log.duration_ms.map(fmt_duration).unwrap_or_else(|| "—".into())}</dd></div>
                            <div><dt>{i18n.t("logs.metrics.source")}</dt><dd>{source}</dd></div>
                            <div><dt>{i18n.t("logs.metrics.messages")}</dt><dd>{format!("{} / {}", log.message_count, log.tool_count)}</dd></div>
                            <div><dt>{i18n.t("logs.metrics.input")}</dt><dd>{fmt_opt(input)}</dd></div>
                            <div><dt>{i18n.t("logs.metrics.output")}</dt><dd>{fmt_opt(log.output_tokens)}</dd></div>
                            <div><dt>{i18n.t("logs.metrics.cacheWrite")}</dt><dd>{fmt_opt(log.cache_creation_input_tokens)}</dd></div>
                            <div><dt>{i18n.t("logs.metrics.cacheRead")}</dt><dd>{fmt_opt(log.cache_read_input_tokens)}<small>{cache_pct}</small></dd></div>
                            <div><dt>{i18n.t("logs.metrics.breakpoints")}</dt><dd>{log.cache_control_breakpoints}</dd></div>
                            <div><dt>{i18n.t("logs.metrics.estimated")}</dt><dd>{fmt_int(log.estimated_context_tokens as u64)}</dd></div>
                            <div><dt>{i18n.t("logs.metrics.errorCode")}</dt><dd>{log.error_code.clone().unwrap_or_else(|| "—".into())}</dd></div>
                            <div><dt>"ID"</dt><dd>{format!("#{}", log.id)}</dd></div>
                        </dl>
                        <p class="log-note">
                            {if input.is_none() { i18n.t("logs.estimateNote") } else { i18n.t("logs.cacheNote") }}
                        </p>
                        {(!error.is_empty()).then(|| view! { <pre class="log-error-detail">{error}</pre> })}
                        <div class="log-detail-actions">
                            <button class="btn btn-ghost btn-xs" on:click=copy_json>{i18n.t("logs.copyJson")}</button>
                        </div>
                    }
                }}
            </div>
        </details>
    }
}

/// Claude reports ordinary input and cache input as disjoint counters; combine only for display.
fn total_input(log: &ApiRequestLog) -> Option<u64> {
    log.input_tokens.map(|input| {
        input
            .saturating_add(log.cache_creation_input_tokens.unwrap_or(0))
            .saturating_add(log.cache_read_input_tokens.unwrap_or(0))
    })
}

fn haystack(log: &ApiRequestLog) -> String {
    format!(
        "#{} {} {} {} {} {} {}",
        log.id,
        log.model,
        log.provider,
        log.api_format,
        log.status,
        log.error_code.as_deref().unwrap_or(""),
        log.error.as_deref().unwrap_or("")
    )
    .to_lowercase()
}

fn status_kind(status: &str) -> &'static str {
    match status {
        "success" => "success",
        "pending" => "pending",
        "timeout" => "timeout",
        "cancelled" => "cancelled",
        "interrupted" => "interrupted",
        _ => "error",
    }
}

fn document_hidden() -> bool {
    web_sys::window()
        .and_then(|w| w.document())
        .map(|d| d.hidden())
        .unwrap_or(false)
}

fn set_all_open(open: bool) {
    let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    let Ok(list) = doc.query_selector_all("details.log-entry") else {
        return;
    };
    for i in 0..list.length() {
        if let Some(el) = list.item(i).and_then(|n| n.dyn_into::<web_sys::Element>().ok()) {
            let _ = if open {
                el.set_attribute("open", "")
            } else {
                el.remove_attribute("open")
            };
        }
    }
}

fn fmt_int(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

fn fmt_opt(value: Option<u64>) -> String {
    value.map(fmt_int).unwrap_or_else(|| "—".into())
}

fn fmt_compact(value: u64) -> String {
    if value >= 1_000_000 {
        format!("{:.1}M", value as f64 / 1e6)
    } else if value >= 10_000 {
        format!("{:.1}k", value as f64 / 1e3)
    } else {
        fmt_int(value)
    }
}

fn fmt_pct(num: u64, den: u64) -> String {
    if den == 0 {
        "—".into()
    } else {
        format!("{:.0}%", num as f64 / den as f64 * 100.0)
    }
}

fn fmt_duration(ms: i64) -> String {
    let ms = ms.max(0);
    if ms < 1000 {
        format!("{ms} ms")
    } else if ms < 60_000 {
        format!("{:.1}s", ms as f64 / 1000.0)
    } else {
        format!("{}m {:02}s", ms / 60_000, (ms % 60_000) / 1000)
    }
}

fn js_date(ts_ms: i64) -> js_sys::Date {
    js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(ts_ms as f64))
}

fn fmt_full(ts_ms: i64) -> String {
    let date = js_date(ts_ms);
    format!(
        "{}-{:02}-{:02} {:02}:{:02}:{:02}",
        date.get_full_year(),
        date.get_month() + 1,
        date.get_date(),
        date.get_hours(),
        date.get_minutes(),
        date.get_seconds()
    )
}

/// Same-day timestamps show the clock only; older ones keep the date.
fn fmt_clock(ts_ms: i64, now_ms: i64) -> String {
    let date = js_date(ts_ms);
    let today = js_date(now_ms);
    let clock = format!(
        "{:02}:{:02}:{:02}",
        date.get_hours(),
        date.get_minutes(),
        date.get_seconds()
    );
    if date.get_full_year() == today.get_full_year()
        && date.get_month() == today.get_month()
        && date.get_date() == today.get_date()
    {
        clock
    } else {
        format!("{:02}-{:02} {clock}", date.get_month() + 1, date.get_date())
    }
}

fn fmt_relative(i18n: I18n, ts_ms: i64, now_ms: i64) -> Option<String> {
    let secs = now_ms.saturating_sub(ts_ms).max(0) / 1000;
    if secs < 10 {
        Some(i18n.t("logs.time.justNow"))
    } else if secs < 60 {
        Some(i18n.tf("logs.time.secondsAgo", &[("n", &secs.to_string())]))
    } else if secs < 3600 {
        Some(i18n.tf("logs.time.minutesAgo", &[("n", &(secs / 60).to_string())]))
    } else if secs < 86_400 {
        Some(i18n.tf("logs.time.hoursAgo", &[("n", &(secs / 3600).to_string())]))
    } else {
        None
    }
}
