use leptos::{ev, prelude::*};
use serde_json::{Map, Value};
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlInputElement;

use crate::{
    api, i18n::use_i18n, storage,
    types::{ConfigData, RequestParamRule},
};

#[component]
pub fn ConfigTab() -> impl IntoView {
    let i18n = use_i18n();
    let config = RwSignal::new(Option::<ConfigData>::None);
    let original_password = RwSignal::new(String::new());
    let original_admin_password = RwSignal::new(String::new());
    let loading = RwSignal::new(true);
    let saving = RwSignal::new(false);
    let error = RwSignal::new(Option::<String>::None);
    let toast = expect_context::<RwSignal<Option<(String, bool)>>>();

    let fetch_config = move || {
        loading.set(true);
        error.set(None);
        spawn_local(async move {
            match api::get_config().await {
                Ok(data) => {
                    original_password.set(data.password.clone());
                    original_admin_password.set(data.admin_password.clone());
                    config.set(Some(data));
                }
                Err(e) => error.set(Some(e)),
            }
            loading.set(false);
        });
    };

    fetch_config();

    let on_save = {
        let i = use_i18n();
        move |_| {
            let Some(cfg) = config.get_untracked() else {
                return;
            };
            saving.set(true);
            error.set(None);
            let orig_pwd = original_password.get_untracked();
            let orig_admin = original_admin_password.get_untracked();
            spawn_local(async move {
                match api::save_config(&cfg).await {
                    Ok(()) => {
                        if cfg.admin_password != orig_admin {
                            toast.set(Some((i.t("config.adminPasswordChanged"), true)));
                            gloo_timers::future::TimeoutFuture::new(3000).await;
                            storage::remove("authToken");
                            let window = web_sys::window().unwrap();
                            let _ = window.location().set_href("/?passwordChanged=true");
                        } else if cfg.password != orig_pwd {
                            toast.set(Some((i.t("config.passwordChanged"), true)));
                        } else {
                            toast.set(Some((i.t("config.success"), true)));
                        }
                    }
                    Err(e) => {
                        error.set(Some(e));
                        toast.set(Some((i.t("config.error"), false)));
                    }
                }
                saving.set(false);
            });
        }
    };

    let set_text = move |name: String, value: String| {
        config.update(|c| {
            let Some(c) = c.as_mut() else { return };
            match name.as_str() {
                "ip" => c.ip = value,
                "port" => c.port = value.parse().unwrap_or(c.port),
                "password" => c.password = value,
                "admin_password" => c.admin_password = value,
                "proxy" => c.proxy = if value.is_empty() { None } else { Some(value) },
                "rproxy" => c.rproxy = if value.is_empty() { None } else { Some(value) },
                "max_retries" => c.max_retries = value.parse().unwrap_or(c.max_retries),
                "custom_h" => c.custom_h = if value.is_empty() { None } else { Some(value) },
                "custom_a" => c.custom_a = if value.is_empty() { None } else { Some(value) },
                "custom_prompt" => c.custom_prompt = value,
                "prompt_cache_anchor_models" => {
                    c.prompt_cache_anchor_models = parse_csv_list(&value)
                }
                "prompt_cache_anchor_text" => c.prompt_cache_anchor_text = value,
                "custom_system" => {
                    c.custom_system = if value.is_empty() { None } else { Some(value) }
                }
                _ => {}
            }
        });
    };

    let set_bool = move |name: String, checked: bool| {
        config.update(|c| {
            let Some(c) = c.as_mut() else { return };
            match name.as_str() {
                "check_update" => c.check_update = checked,
                "auto_update" => c.auto_update = checked,
                "preserve_chats" => c.preserve_chats = checked,
                "web_search" => c.web_search = checked,
                "enable_web_count_tokens" => c.enable_web_count_tokens = checked,
                "sanitize_messages" => c.sanitize_messages = checked,
                "prompt_cache_anchor_enabled" => c.prompt_cache_anchor_enabled = checked,
                "skip_first_warning" => c.skip_first_warning = checked,
                "skip_second_warning" => c.skip_second_warning = checked,
                "skip_restricted" => c.skip_restricted = checked,
                "skip_non_pro" => c.skip_non_pro = checked,
                "skip_rate_limit" => c.skip_rate_limit = checked,
                "skip_normal_pro" => c.skip_normal_pro = checked,
                "use_real_roles" => c.use_real_roles = checked,
                _ => {}
            }
        });
    };

    let on_input = move |ev: ev::Event| {
        let target = event_target::<HtmlInputElement>(&ev);
        set_text(target.name(), target.value());
    };

    let on_checkbox = move |ev: ev::Event| {
        let target = event_target::<HtmlInputElement>(&ev);
        set_bool(target.name(), target.checked());
    };

    let on_textarea = move |ev: ev::Event| {
        let target = event_target::<web_sys::HtmlTextAreaElement>(&ev);
        set_text(target.name(), target.value());
    };

    view! {
        <div class="stack">
            <Show when=move || loading.get()>
                <p class="loading">{move || i18n.t("common.loading")}</p>
            </Show>

            <Show when=move || error.get().is_some()>
                <div class="alert alert-error">
                    {move || error.get().unwrap_or_default()}
                    <button
                        class="link"
                        style="margin-left:0.5rem"
                        on:click=move |_| fetch_config()
                    >
                        {move || i18n.t("config.retry")}
                    </button>
                </div>
            </Show>

            <Show when=move || config.get().is_some()>
                {move || {
                    let cfg = config.get().unwrap();
                    view! {
                        <div class="stack-lg">
                            <div class="row-btw">
                                <h3>{i18n.t("config.title")}</h3>
                                <button
                                    class="btn btn-primary btn-sm"
                                    disabled=move || saving.get()
                                    on:click=on_save
                                >
                                    {move || {
                                        if saving.get() {
                                            i18n.t("config.saving")
                                        } else {
                                            i18n.t("config.saveButton")
                                        }
                                    }}
                                </button>
                            </div>

                            <div class="stack">
                                // Server
                                <ConfigSection
                                    title=i18n.t("config.sections.server.title")
                                    description=i18n.t("config.sections.server.description")
                                >
                                    <div class="grid-2">
                                        <TextInput name="ip" label=i18n.t("config.sections.server.ip") value=cfg.ip.clone() on_input=on_input />
                                        <TextInput name="port" label=i18n.t("config.sections.server.port") value=cfg.port.to_string() input_type="number" on_input=on_input />
                                    </div>
                                </ConfigSection>

                                // App
                                <ConfigSection title=i18n.t("config.sections.app.title")>
                                    <div class="row-lg">
                                        <Checkbox name="check_update" label=i18n.t("config.sections.app.checkUpdate") checked=cfg.check_update on_input=on_checkbox />
                                        <Checkbox name="auto_update" label=i18n.t("config.sections.app.autoUpdate") checked=cfg.auto_update on_input=on_checkbox />
                                    </div>
                                </ConfigSection>

                                // Network
                                <ConfigSection title=i18n.t("config.sections.network.title")>
                                    <TextInput name="password" label=i18n.t("config.sections.network.password") value=cfg.password.clone() input_type="password" on_input=on_input />
                                    <TextInput name="admin_password" label=i18n.t("config.sections.network.adminPassword") value=cfg.admin_password.clone() input_type="password" on_input=on_input />
                                    <TextInput name="proxy" label=i18n.t("config.sections.network.proxy") value=cfg.proxy.clone().unwrap_or_default() on_input=on_input />
                                    <TextInput name="rproxy" label=i18n.t("config.sections.network.rproxy") value=cfg.rproxy.clone().unwrap_or_default() on_input=on_input />
                                </ConfigSection>

                                // API
                                <ConfigSection title=i18n.t("config.sections.api.title")>
                                    <TextInput name="max_retries" label=i18n.t("config.sections.api.maxRetries") value=cfg.max_retries.to_string() input_type="number" on_input=on_input />
                                    <div class="grid-2" style="margin-top:0.5rem">
                                        <Checkbox name="preserve_chats" label=i18n.t("config.sections.api.preserveChats") checked=cfg.preserve_chats on_input=on_checkbox />
                                        <Checkbox name="web_search" label=i18n.t("config.sections.api.webSearch") checked=cfg.web_search on_input=on_checkbox />
                                        <Checkbox name="enable_web_count_tokens" label=i18n.t("config.sections.api.webCountTokens") checked=cfg.enable_web_count_tokens on_input=on_checkbox />
                                        <Checkbox name="sanitize_messages" label=i18n.t("config.sections.api.sanitizeMessages") checked=cfg.sanitize_messages on_input=on_checkbox />
                                    </div>
                                    <div class="stack" style="margin-top:0.75rem">
                                        <div class="stack-sm">
                                            <Checkbox name="prompt_cache_anchor_enabled" label=i18n.t("config.sections.api.promptCacheAnchorEnabled") checked=cfg.prompt_cache_anchor_enabled on_input=on_checkbox />
                                            <div class="config-section-desc" style="margin-bottom:0">
                                                {i18n.t("config.sections.api.promptCacheAnchorHint")}
                                            </div>
                                        </div>
                                        <div class="grid-2">
                                            <TextInput
                                                name="prompt_cache_anchor_models"
                                                label=i18n.t("config.sections.api.promptCacheAnchorModels")
                                                value=join_csv_list(&cfg.prompt_cache_anchor_models)
                                                on_input=on_input
                                            />
                                            <TextInput
                                                name="prompt_cache_anchor_text"
                                                label=i18n.t("config.sections.api.promptCacheAnchorText")
                                                value=cfg.prompt_cache_anchor_text.clone()
                                                on_input=on_input
                                            />
                                        </div>
                                    </div>
                                    <RequestParamRulesEditor config=config />
                                </ConfigSection>

                                // Cookie
                                <ConfigSection title=i18n.t("config.sections.cookie.title")>
                                    <div class="stack-sm">
                                        <Checkbox name="skip_non_pro" label=i18n.t("config.sections.cookie.skipFree") checked=cfg.skip_non_pro on_input=on_checkbox />
                                        <Checkbox name="skip_restricted" label=i18n.t("config.sections.cookie.skipRestricted") checked=cfg.skip_restricted on_input=on_checkbox />
                                        <Checkbox name="skip_second_warning" label=i18n.t("config.sections.cookie.skipSecondWarning") checked=cfg.skip_second_warning on_input=on_checkbox />
                                        <Checkbox name="skip_first_warning" label=i18n.t("config.sections.cookie.skipFirstWarning") checked=cfg.skip_first_warning on_input=on_checkbox />
                                        <Checkbox name="skip_normal_pro" label=i18n.t("config.sections.cookie.skipNormalPro") checked=cfg.skip_normal_pro on_input=on_checkbox />
                                        <Checkbox name="skip_rate_limit" label=i18n.t("config.sections.cookie.skipRateLimit") checked=cfg.skip_rate_limit on_input=on_checkbox />
                                    </div>
                                </ConfigSection>

                                // Prompt
                                <ConfigSection title=i18n.t("config.sections.prompt.title")>
                                    <Checkbox name="use_real_roles" label=i18n.t("config.sections.prompt.realRoles") checked=cfg.use_real_roles on_input=on_checkbox />
                                    <TextInput name="custom_h" label=i18n.t("config.sections.prompt.customH") value=cfg.custom_h.clone().unwrap_or_default() on_input=on_input />
                                    <TextInput name="custom_a" label=i18n.t("config.sections.prompt.customA") value=cfg.custom_a.clone().unwrap_or_default() on_input=on_input />
                                    <TextArea name="custom_prompt" label=i18n.t("config.sections.prompt.customPrompt") value=cfg.custom_prompt.clone() on_input=on_textarea />
                                    <TextArea name="custom_system" label=i18n.t("config.sections.prompt.customSystem") value=cfg.custom_system.clone().unwrap_or_default() on_input=on_textarea />
                                </ConfigSection>
                            </div>
                        </div>
                    }
                }}
            </Show>
        </div>
    }
}

#[component]
fn ConfigSection(
    title: String,
    #[prop(optional)] description: Option<String>,
    children: Children,
) -> impl IntoView {
    view! {
        <div class="config-section">
            <div class="config-section-title">{title}</div>
            {description.map(|d| {
                view! { <p class="config-section-desc">{d}</p> }
            })}
            {children()}
        </div>
    }
}

fn parse_csv_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect()
}

fn join_csv_list(values: &[String]) -> String {
    values.join(", ")
}

fn params_to_text(params: &Map<String, Value>) -> String {
    if params.is_empty() {
        "{\n}".to_string()
    } else {
        serde_json::to_string_pretty(params).unwrap_or_else(|_| "{\n}".to_string())
    }
}

#[component]
fn RequestParamRulesEditor(config: RwSignal<Option<ConfigData>>) -> impl IntoView {
    let i18n = use_i18n();

    let add_rule = move |_| {
        config.update(|cfg| {
            let Some(cfg) = cfg.as_mut() else { return };
            cfg.request_param_rules.push(RequestParamRule {
                models: vec!["claude-fable-*".to_string()],
                exclude: Vec::new(),
                params: Map::new(),
            });
        });
    };

    view! {
        <div class="stack" style="margin-top:0.75rem">
            <div class="row-btw">
                <div class="stack-sm">
                    <div class="label-sm">{move || i18n.t("config.sections.api.requestParamRules")}</div>
                    <div class="config-section-desc" style="margin-bottom:0">
                        {move || i18n.t("config.sections.api.requestParamRulesHint")}
                    </div>
                </div>
                <button class="btn btn-ghost btn-sm" on:click=add_rule>
                    {move || i18n.t("config.sections.api.addRule")}
                </button>
            </div>

            <Show
                when=move || {
                    config
                        .get()
                        .is_some_and(|cfg| !cfg.request_param_rules.is_empty())
                }
                fallback=move || {
                    view! {
                        <div class="rule-empty">
                            {move || i18n.t("config.sections.api.noRequestParamRules")}
                        </div>
                    }
                }
            >
                <For
                    each=move || {
                        config
                            .get()
                            .map(|cfg| {
                                cfg.request_param_rules
                                    .iter()
                                    .cloned()
                                    .enumerate()
                                    .collect::<Vec<_>>()
                            })
                            .unwrap_or_default()
                    }
                    key=|(index, _)| *index
                    children=move |(index, rule)| {
                        let on_models = move |ev: ev::Event| {
                            let value = event_target::<HtmlInputElement>(&ev).value();
                            config.update(|cfg| {
                                let Some(cfg) = cfg.as_mut() else { return };
                                let Some(rule) = cfg.request_param_rules.get_mut(index) else {
                                    return;
                                };
                                rule.models = parse_csv_list(&value);
                            });
                        };

                        let on_exclude = move |ev: ev::Event| {
                            let value = event_target::<HtmlInputElement>(&ev).value();
                            config.update(|cfg| {
                                let Some(cfg) = cfg.as_mut() else { return };
                                let Some(rule) = cfg.request_param_rules.get_mut(index) else {
                                    return;
                                };
                                rule.exclude = parse_csv_list(&value);
                            });
                        };

                        let on_params = move |ev: ev::Event| {
                            let value = event_target::<web_sys::HtmlTextAreaElement>(&ev).value();
                            config.update(|cfg| {
                                let Some(cfg) = cfg.as_mut() else { return };
                                let Some(rule) = cfg.request_param_rules.get_mut(index) else {
                                    return;
                                };
                                if value.trim().is_empty() {
                                    rule.params = Map::new();
                                    return;
                                }
                                if let Ok(Value::Object(params)) = serde_json::from_str::<Value>(&value) {
                                    rule.params = params;
                                }
                            });
                        };

                        let remove_rule = move |_| {
                            config.update(|cfg| {
                                let Some(cfg) = cfg.as_mut() else { return };
                                if index < cfg.request_param_rules.len() {
                                    cfg.request_param_rules.remove(index);
                                }
                            });
                        };

                        view! {
                            <div class="rule-card">
                                <div class="row-btw">
                                    <div class="rule-title">
                                        {move || i18n.t("config.sections.api.ruleTitle")}
                                        {" "}
                                        {index + 1}
                                    </div>
                                    <button class="btn btn-danger btn-xs" on:click=remove_rule>
                                        {move || i18n.t("config.sections.api.removeRule")}
                                    </button>
                                </div>

                                <div class="grid-2">
                                    <div class="stack-sm">
                                        <label class="label-sm">
                                            {move || i18n.t("config.sections.api.ruleModels")}
                                        </label>
                                        <input
                                            class="input input-sm"
                                            value=join_csv_list(&rule.models)
                                            placeholder="claude-fable-*, claude-opus-*"
                                            on:input=on_models
                                        />
                                    </div>

                                    <div class="stack-sm">
                                        <label class="label-sm">
                                            {move || i18n.t("config.sections.api.ruleExclude")}
                                        </label>
                                        <input
                                            class="input input-sm"
                                            value=join_csv_list(&rule.exclude)
                                            placeholder="reasoning_effort, thinking, top_p"
                                            on:input=on_exclude
                                        />
                                    </div>
                                </div>

                                <div class="stack-sm">
                                    <label class="label-sm">
                                        {move || i18n.t("config.sections.api.ruleParams")}
                                    </label>
                                    <textarea
                                        class="textarea"
                                        rows="5"
                                        on:input=on_params
                                    >
                                        {params_to_text(&rule.params)}
                                    </textarea>
                                </div>
                            </div>
                        }
                    }
                />
            </Show>
        </div>
    }
}

#[component]
fn TextInput(
    name: &'static str,
    label: String,
    value: String,
    #[prop(default = "text")] input_type: &'static str,
    on_input: impl Fn(ev::Event) + Copy + 'static,
) -> impl IntoView {
    view! {
        <div class="stack-sm">
            <label class="label-sm">{label}</label>
            <input
                type=input_type
                name=name
                value=value
                on:input=on_input
                class="input input-sm"
            />
        </div>
    }
}

#[component]
fn TextArea(
    name: &'static str,
    label: String,
    value: String,
    #[prop(default = "3")] rows: &'static str,
    on_input: impl Fn(ev::Event) + Copy + 'static,
) -> impl IntoView {
    view! {
        <div class="stack-sm">
            <label class="label-sm">{label}</label>
            <textarea
                name=name
                rows=rows
                on:input=on_input
                class="textarea"
            >
                {value}
            </textarea>
        </div>
    }
}

#[component]
fn Checkbox(
    name: &'static str,
    label: String,
    checked: bool,
    on_input: impl Fn(ev::Event) + Copy + 'static,
) -> impl IntoView {
    view! {
        <label class="checkbox-row">
            <input
                type="checkbox"
                name=name
                checked=checked
                on:change=on_input
            />
            {label}
        </label>
    }
}
