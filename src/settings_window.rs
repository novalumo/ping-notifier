//! 設定画面。gpui-component の `Settings` に項目を並べ、「保存」で設定ファイルを書き換える。
//!
//! 各欄は値を `App` から読み書きする作りなので、編集中の値（下書き）は `Global` に置く。
//! 下書きは「保存」を押すまで設定ファイルにも監視にも反映しない。

use std::path::PathBuf;

use futures::channel::mpsc::UnboundedSender;
use gpui::{
    App, AppContext as _, Axis, BorrowAppContext as _, Bounds, Context, Global, IntoElement,
    ParentElement as _, Render, SharedString, Styled as _, TitlebarOptions, Window, WindowBounds,
    WindowHandle, WindowOptions, div, prelude::FluentBuilder as _, px, size,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::setting::{
    NumberFieldOptions, SettingField, SettingGroup, SettingItem, SettingPage, Settings,
};
use gpui_component::{ActiveTheme as _, Root, Theme, h_flex, v_flex};

use crate::UserEvent;
use crate::config::{self, Config};
use crate::i18n::{self, Lang, LanguageSetting, Msg, t};

/// 「遅延を通知する」をオンにしたときに入れるしきい値（テンプレートのコメントと同じ値）
const DEFAULT_LATENCY_THRESHOLD_MS: u64 = 200;

/// 編集中の設定
struct Draft {
    config: Config,
    /// 「遅延を通知する」がオフのあいだも、入力したしきい値を覚えておく
    latency_enabled: bool,
    latency_threshold_ms: u64,
}

impl Global for Draft {}

impl Draft {
    fn new(config: Config) -> Self {
        Self {
            latency_enabled: config.latency_threshold_ms.is_some(),
            latency_threshold_ms: config
                .latency_threshold_ms
                .unwrap_or(DEFAULT_LATENCY_THRESHOLD_MS),
            config,
        }
    }

    fn to_config(&self) -> Config {
        Config {
            latency_threshold_ms: self.latency_enabled.then_some(self.latency_threshold_ms),
            ..self.config.clone()
        }
    }
}

pub struct SettingsWindow {
    config_path: PathBuf,
    events: UnboundedSender<UserEvent>,
    error: Option<SharedString>,
}

/// 設定画面を開く。`config` を下書きの初期値にする
pub fn open(
    config: Config,
    config_path: PathBuf,
    events: UnboundedSender<UserEvent>,
    cx: &mut App,
) -> anyhow::Result<WindowHandle<Root>> {
    cx.set_global(Draft::new(config));
    sync_component_language();
    Theme::sync_system_appearance(None, cx);

    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(680.), px(540.)),
            cx,
        ))),
        titlebar: Some(TitlebarOptions {
            title: Some(t(Msg::SettingsTitle).into()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let handle = cx.open_window(options, |window, cx| {
        let view = cx.new(|cx| {
            // 下書きが変わったら描き直す（「遅延を通知する」に合わせて欄を有効・無効にするため）
            cx.observe_global::<Draft>(|_, cx| cx.notify()).detach();
            SettingsWindow {
                config_path,
                events,
                error: None,
            }
        });
        cx.new(|cx| Root::new(view, window, cx))
    })?;
    // メニューバー常駐アプリ（Accessory）なので、明示的に前面へ出す
    cx.activate(true);
    Ok(handle)
}

/// gpui-component の部品（検索欄など）の文言を表示言語に合わせる。対応していない言語は英語
fn sync_component_language() {
    gpui_component::set_locale(match i18n::current() {
        Lang::Ja => "ja",
        Lang::Zh => "zh-CN",
        Lang::En | Lang::Ko | Lang::Eo | Lang::De => "en",
    });
}

fn draft(cx: &App) -> &Draft {
    cx.global::<Draft>()
}

/// 下書きを書き換える。`update_global` を使うので、`observe_global` している画面が描き直される
fn edit(cx: &mut App, f: impl FnOnce(&mut Draft)) {
    cx.update_global::<Draft, _>(|draft, _| f(draft));
}

/// 数値欄の値を正の整数にそろえる（欄は f64 で値を返す）
fn to_count(value: f64) -> u64 {
    value.round().max(1.0) as u64
}

fn number(min: f64, step: f64) -> NumberFieldOptions {
    NumberFieldOptions {
        min,
        max: f64::MAX,
        step,
    }
}

fn monitoring_page() -> SettingPage {
    SettingPage::new(t(Msg::SettingsPageMonitoring)).group(
        SettingGroup::new()
            .item(
                SettingItem::new(
                    t(Msg::SettingsHost),
                    SettingField::input(
                        |cx| draft(cx).config.host.clone().into(),
                        |value, cx| edit(cx, |d| d.config.host = value.trim().to_owned()),
                    ),
                )
                .description(t(Msg::SettingsHostDescription))
                // 入力欄を説明の横に並べると幅が足りないので、下に置いて横幅いっぱいに使う
                .layout(Axis::Vertical),
            )
            .item(SettingItem::new(
                t(Msg::SettingsInterval),
                SettingField::number_input(
                    number(0.1, 0.5),
                    |cx| draft(cx).config.interval_secs,
                    |value, cx| edit(cx, |d| d.config.interval_secs = value),
                ),
            ))
            .item(
                SettingItem::new(
                    t(Msg::SettingsTimeout),
                    SettingField::number_input(
                        number(1.0, 100.0),
                        |cx| draft(cx).config.timeout_ms as f64,
                        |value, cx| edit(cx, |d| d.config.timeout_ms = to_count(value)),
                    ),
                )
                .description(t(Msg::SettingsTimeoutDescription)),
            )
            .item(SettingItem::new(
                t(Msg::SettingsThreshold),
                SettingField::number_input(
                    number(1.0, 1.0),
                    |cx| f64::from(draft(cx).config.threshold),
                    |value, cx| edit(cx, |d| d.config.threshold = to_count(value) as u32),
                ),
            )),
    )
}

fn notifications_page(cx: &App) -> SettingPage {
    // しきい値と回数は「遅延を通知する」がオンのときだけ編集できる
    let latency_disabled = !draft(cx).latency_enabled;
    SettingPage::new(t(Msg::SettingsPageNotifications))
        .group(
            SettingGroup::new()
                .item(SettingItem::new(
                    t(Msg::SettingsNotifyRecovery),
                    SettingField::switch(
                        |cx| draft(cx).config.notify_recovery,
                        |value, cx| edit(cx, |d| d.config.notify_recovery = value),
                    ),
                ))
                .item(SettingItem::new(
                    t(Msg::SettingsLossSound),
                    SettingField::switch(
                        |cx| draft(cx).config.loss_sound,
                        |value, cx| edit(cx, |d| d.config.loss_sound = value),
                    ),
                )),
        )
        .group(
            SettingGroup::new()
                .item(
                    SettingItem::new(
                        t(Msg::SettingsLatency),
                        SettingField::switch(
                            |cx| draft(cx).latency_enabled,
                            |value, cx| edit(cx, |d| d.latency_enabled = value),
                        ),
                    )
                    .description(t(Msg::SettingsLatencyDescription)),
                )
                .item(
                    SettingItem::new(
                        t(Msg::SettingsLatencyThreshold),
                        SettingField::number_input(
                            number(1.0, 50.0),
                            |cx| draft(cx).latency_threshold_ms as f64,
                            |value, cx| edit(cx, |d| d.latency_threshold_ms = to_count(value)),
                        ),
                    )
                    .disabled(latency_disabled),
                )
                .item(
                    SettingItem::new(
                        t(Msg::SettingsLatencyConsecutive),
                        SettingField::number_input(
                            number(1.0, 1.0),
                            |cx| f64::from(draft(cx).config.latency_consecutive),
                            |value, cx| {
                                edit(cx, |d| {
                                    d.config.latency_consecutive = to_count(value) as u32
                                })
                            },
                        ),
                    )
                    .disabled(latency_disabled),
                ),
        )
}

fn language_label(setting: LanguageSetting) -> SharedString {
    match setting {
        LanguageSetting::Auto => t(Msg::SettingsLanguageAuto).into(),
        // 各言語の名前は、その言語自身の表記で出す
        LanguageSetting::En => "English".into(),
        LanguageSetting::Ja => "日本語".into(),
        LanguageSetting::Zh => "中文（简体）".into(),
        LanguageSetting::Ko => "한국어".into(),
        LanguageSetting::Eo => "Esperanto".into(),
        LanguageSetting::De => "Deutsch".into(),
    }
}

fn general_page() -> SettingPage {
    let languages = LanguageSetting::ALL
        .into_iter()
        .map(|setting| (setting.as_str().into(), language_label(setting)))
        .collect();
    SettingPage::new(t(Msg::SettingsPageGeneral)).group(
        SettingGroup::new()
            .item(SettingItem::new(
                t(Msg::SettingsLanguage),
                SettingField::dropdown(
                    languages,
                    |cx| draft(cx).config.language.as_str().into(),
                    |value: SharedString, cx| {
                        if let Some(setting) = LanguageSetting::ALL
                            .into_iter()
                            .find(|s| s.as_str() == value.as_ref())
                        {
                            edit(cx, |d| d.config.language = setting);
                        }
                    },
                ),
            ))
            .item(SettingItem::new(
                t(Msg::SettingsAutoUpdate),
                SettingField::switch(
                    |cx| draft(cx).config.auto_update,
                    |value, cx| edit(cx, |d| d.config.auto_update = value),
                ),
            )),
    )
}

impl SettingsWindow {
    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let config = draft(cx).to_config();
        match config::save(&self.config_path, &config) {
            Ok(()) => {
                crate::log("saved settings from the settings window");
                // 反映は「設定を再読み込み」と同じ処理を UI スレッドで行う
                let _ = self.events.unbounded_send(UserEvent::SettingsSaved);
                window.remove_window();
            }
            Err(e) => {
                crate::log(&format!("failed to save settings: {e:#}"));
                self.error = Some(format!("{}: {e:#}", t(Msg::SettingsSaveFailed)).into());
                cx.notify();
            }
        }
    }
}

impl Render for SettingsWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let (danger, border, background) = (theme.danger, theme.border, theme.background);
        v_flex()
            .size_full()
            .bg(background)
            .child(div().flex_1().min_h_0().child(
                Settings::new("settings").sidebar_width(px(170.)).pages([
                    monitoring_page(),
                    notifications_page(cx),
                    general_page(),
                ]),
            ))
            .child(
                h_flex()
                    .gap_2()
                    .px_4()
                    .py_3()
                    .border_t_1()
                    .border_color(border)
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .text_color(danger)
                            .when_some(self.error.clone(), |this, error| this.child(error)),
                    )
                    .child(
                        Button::new("cancel")
                            .label(t(Msg::SettingsCancel))
                            .on_click(|_, window, _| window.remove_window()),
                    )
                    .child(
                        Button::new("save")
                            .primary()
                            .label(t(Msg::SettingsSave))
                            .on_click(cx.listener(|this, _, window, cx| this.save(window, cx))),
                    ),
            )
    }
}
