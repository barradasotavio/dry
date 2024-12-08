use std::{collections::HashMap, path::PathBuf};

use pyo3::{types::PyFunction, Py};
use tao::{event_loop::EventLoopProxy, window::Window};
use wry::{
    http::Request, Error as WryError, WebContext, WebView, WebViewBuilder,
};

use crate::{
    api::{handle_api_requests, API_JS},
    events::AppEvent,
    window::{
        handle_window_requests, WINDOW_BORDERS_JS, WINDOW_EVENTS_JS,
        WINDOW_FUNCTIONS_JS,
    },
};

pub fn build_webview(
    window: &Window,
    ipc_handler: impl Fn(Request<String>) + 'static,
    html: Option<String>,
    url: Option<String>,
    decorations: bool,
    api: bool,
    dev_tools: bool,
    udf: String,
) -> Result<WebView, WryError> {
    let data_directory = PathBuf::from(udf);
    let mut web_context = WebContext::new(Some(data_directory));

    let mut builder = WebViewBuilder::with_web_context(&mut web_context)
        .with_initialization_script(WINDOW_FUNCTIONS_JS)
        .with_initialization_script(WINDOW_EVENTS_JS)
        .with_devtools(dev_tools)
        .with_ipc_handler(ipc_handler);

    if api {
        builder = builder.with_initialization_script(API_JS);
    }

    if !decorations {
        builder = builder.with_initialization_script(WINDOW_BORDERS_JS);
    }

    builder = match (html, url) {
        (Some(html), _) => builder.with_html(html),
        (None, Some(url)) => builder.with_url(url),
        (None, None) => panic!("No html or url provided."),
    };

    let webview = builder.build(window)?;

    Ok(webview)
}

pub fn build_ipc_handler(
    api: Option<HashMap<String, Py<PyFunction>>>,
    event_loop_proxy: EventLoopProxy<AppEvent>,
) -> impl Fn(Request<String>) + 'static {
    move |request| {
        let request_body = request.body();

        if request_body.starts_with("window_control") {
            handle_window_requests(request_body, &event_loop_proxy);
            return;
        }

        if let Some(api) = &api {
            if let Err(err) =
                handle_api_requests(request_body, api, &event_loop_proxy)
            {
                eprintln!("{:?}", err);
            }
        }
    }
}
