// Chapter picker shared by the local player and the Music Assistant speaker
// bar. The caller owns the position and decides what a seek or toggle does.

use std::rc::Rc;

use i18nrs::yew::use_translation;
use yew::prelude::*;

use crate::components::gen_funcs::format_time_rm_hour;
use crate::requests::pod_req::Chapter;
use crate::requests::podcasting2::{chapter_bounds, chapter_progress, http_link};

#[derive(Properties, PartialEq)]
pub(crate) struct ChapterListProps {
    pub(crate) chapters: Rc<Vec<Chapter>>,
    pub(crate) position: f64,
    pub(crate) duration: f64,
    pub(crate) playing: bool,
    /// Chapter art goes through this server's image proxy when it is set.
    pub(crate) server_name: Option<String>,
    /// Seek to a chapter start, in seconds.
    pub(crate) on_seek: Callback<f64>,
    /// Tapping the current chapter plays or pauses instead.
    pub(crate) on_toggle: Callback<()>,
    pub(crate) on_close: Callback<()>,
}

#[function_component(ChapterList)]
pub(crate) fn chapter_list(props: &ChapterListProps) -> Html {
    let (i18n, _) = use_translation();
    let close = props.on_close.reform(|_: MouseEvent| ());
    let stop_propagation = Callback::from(|e: MouseEvent| e.stop_propagation());
    let bounds = chapter_bounds(&props.chapters, props.duration);

    html! {
        <div id="chapter-select-modal" tabindex="-1" aria-hidden="true"
            class="chapter-select-modal fixed top-0 right-0 left-0 flex justify-center items-center w-full h-[calc(100%-1rem)] max-h-full bg-black bg-opacity-25"
            onclick={close.clone()}>
            <div class="modal-container relative p-4 w-full max-w-md max-h-full rounded-lg shadow"
                onclick={stop_propagation}>
                <div class="modal-container relative rounded-lg shadow">
                    <div class="flex items-center justify-between p-4 md:p-5 border-b rounded-t">
                        <h3 class="text-xl font-semibold">{ i18n.t("audio.chapters") }</h3>
                        <button onclick={close}
                            class="end-2.5 text-gray-400 bg-transparent hover:bg-gray-200 hover:text-gray-900 rounded-lg text-sm w-8 h-8 ms-auto inline-flex justify-center items-center">
                            <svg class="w-3 h-3" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 14 14">
                                <path stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="m1 1 6 6m0 0 6 6M7 7l6-6M7 7l-6 6"/>
                            </svg>
                            <span class="sr-only">{ i18n.t("common.close_modal") }</span>
                        </button>
                    </div>

                    <div class="p-4 md:p-5 max-h-[70vh] overflow-y-auto">
                        if props.chapters.is_empty() {
                            <div class="text-center p-4">{ i18n.t("audio.no_chapters_available") }</div>
                        }
                        { for props.chapters.iter().zip(bounds).map(|(chapter, (start, end))| {
                            let is_current = props.position >= start && props.position < end;
                            let progress = chapter_progress(props.position, start, end);
                            let activate = {
                                let on_seek = props.on_seek.clone();
                                let on_toggle = props.on_toggle.clone();
                                Callback::from(move |e: MouseEvent| {
                                    e.stop_propagation();
                                    if is_current {
                                        on_toggle.emit(());
                                    } else {
                                        on_seek.emit(start);
                                    }
                                })
                            };
                            let thumb = chapter.img.as_deref().and_then(http_link).map(|img| {
                                props.server_name.as_deref().map(|server| {
                                    format!("{}/api/proxy/image?url={}", server, urlencoding::encode(img))
                                }).unwrap_or_else(|| img.to_string())
                            });
                            html! {
                                <div
                                    class={classes!("chapter-item", is_current.then_some("current-chapter"))}
                                    onclick={activate.clone()}
                                >
                                    <button class="chapter-play-button" onclick={activate}>
                                        if is_current && props.playing {
                                            <i class="ph ph-pause text-xl"></i>
                                        } else {
                                            <i class="ph ph-play text-xl"></i>
                                        }
                                    </button>
                                    <div class="chapter-info">
                                        if let Some(src) = thumb {
                                            <img src={src} class="ep-chapter-thumb" alt="" />
                                        }
                                        <span class="chapter-title">{ &chapter.title }</span>
                                        if let Some(href) = chapter.url.as_deref().and_then(http_link) {
                                            <a class="ep-chapter-link" href={href.to_string()} target="_blank" rel="noopener noreferrer"
                                                title={i18n.t("podcasting2.chapter_link").to_string()}
                                                onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}>
                                                <i class="ph ph-arrow-square-out"></i>
                                            </a>
                                        }
                                        <span class="chapter-time">{ format_time_rm_hour(start as i32) }</span>
                                        <div class="chapter-progress-container">
                                            <div class="chapter-progress-bar" style={format!("width: {}%", progress)} />
                                        </div>
                                    </div>
                                </div>
                            }
                        }) }
                    </div>
                </div>
            </div>
        </div>
    }
}
