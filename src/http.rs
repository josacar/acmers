use std::collections::HashMap;
use std::sync::LazyLock;

pub static CLIENT: LazyLock<HttpClient> = LazyLock::new(HttpClient::new);

pub static TEST_BASE_URL: LazyLock<std::sync::Mutex<Option<String>>> = LazyLock::new(|| std::sync::Mutex::new(None));

pub fn set_test_base(url: &str) {
    *TEST_BASE_URL.lock().unwrap() = Some(url.to_string());
}

fn rewrite_url(url: &str) -> String {
    if let Some(base) = TEST_BASE_URL.lock().unwrap().as_ref() {
        let parsed = url.split('/').collect::<Vec<_>>();
        if parsed.len() >= 4 {
            let path: String = parsed[3..].join("/");
            return format!("{base}/{path}");
        }
        return base.clone();
    }
    url.to_string()
}

pub struct HttpClient {
    agent: ureq::Agent,
}

impl HttpClient {
    pub fn new() -> Self {
        let config = ureq::config::Config::builder()
            .http_status_as_error(false)
            .build();
        let agent = ureq::Agent::new_with_config(config);
        HttpClient { agent }
    }

    pub fn get(&self, url: &str, headers: &[(&str, &str)]) -> Result<Response, String> {
        let url = rewrite_url(url);
        let mut req = self.agent.get(&url);
        for (k, v) in headers {
            req = req.header(*k, *v);
        }
        let resp = req.call().map_err(|e| format!("HTTP GET {url}: {e}"))?;
        extract_response(resp)
    }

    pub fn head(&self, url: &str) -> Result<Response, String> {
        let url = rewrite_url(url);
        let resp = self.agent.head(&url).call()
            .map_err(|e| format!("HTTP HEAD {url}: {e}"))?;
        extract_response(resp)
    }

    pub fn post(&self, url: &str, body: &[u8], content_type: &str, headers: &[(&str, &str)]) -> Result<Response, String> {
        let url = rewrite_url(url);
        let mut req = self.agent.post(&url).header("Content-Type", content_type);
        for (k, v) in headers {
            req = req.header(*k, *v);
        }
        let resp = req.send(body).map_err(|e| format!("HTTP POST {url}: {e}"))?;
        extract_response(resp)
    }

    pub fn put(&self, url: &str, body: &[u8], content_type: &str, headers: &[(&str, &str)]) -> Result<Response, String> {
        let url = rewrite_url(url);
        let mut req = self.agent.put(&url).header("Content-Type", content_type);
        for (k, v) in headers {
            req = req.header(*k, *v);
        }
        let resp = req.send(body).map_err(|e| format!("HTTP PUT {url}: {e}"))?;
        extract_response(resp)
    }

    pub fn delete(&self, url: &str, headers: &[(&str, &str)]) -> Result<Response, String> {
        let url = rewrite_url(url);
        let mut req = ureq::delete(&url);
        for (k, v) in headers {
            req = req.header(*k, *v);
        }
        let resp = req.call().map_err(|e| format!("HTTP DELETE {url}: {e}"))?;
        extract_response(resp)
    }

    pub fn delete_with_body(&self, url: &str, body: &[u8], content_type: &str, headers: &[(&str, &str)]) -> Result<Response, String> {
        let url = rewrite_url(url);
        let mut builder = ureq::http::Request::builder()
            .method("DELETE")
            .uri(&url)
            .header("Content-Type", content_type);
        for (k, v) in headers {
            builder = builder.header(*k, *v);
        }
        let req = builder.body(body.to_vec()).map_err(|e| format!("HTTP DELETE {url}: {e}"))?;
        let resp = self.agent.run(req).map_err(|e| format!("HTTP DELETE {url}: {e}"))?;
        extract_response(resp)
    }

    pub fn patch(&self, url: &str, body: &[u8], content_type: &str, headers: &[(&str, &str)]) -> Result<Response, String> {
        let url = rewrite_url(url);
        let mut req = ureq::patch(&url).header("Content-Type", content_type);
        for (k, v) in headers {
            req = req.header(*k, *v);
        }
        let resp = req.send(body).map_err(|e| format!("HTTP PATCH {url}: {e}"))?;
        extract_response(resp)
    }
}

#[derive(Clone)]
pub struct Response {
    pub status: u16,
    pub body: String,
    pub headers: HashMap<String, String>,
}

fn extract_response(resp: ureq::http::Response<ureq::Body>) -> Result<Response, String> {
    let status = resp.status().as_u16();
    let mut headers = HashMap::new();
    for (name, value) in resp.headers().iter() {
        if let Ok(val) = value.to_str() {
            headers.insert(name.as_str().to_lowercase(), val.to_string());
        }
    }
    let body = resp.into_body().read_to_string()
        .map_err(|e| format!("read body: {e}"))?;
    Ok(Response { status, body, headers })
}

pub fn get(url: &str, headers: &[(&str, &str)]) -> Result<Response, String> {
    CLIENT.get(url, headers)
}

pub fn head(url: &str) -> Result<Response, String> {
    CLIENT.head(url)
}

pub fn post(url: &str, body: &[u8], content_type: &str, headers: &[(&str, &str)]) -> Result<Response, String> {
    CLIENT.post(url, body, content_type, headers)
}

pub fn put(url: &str, body: &[u8], content_type: &str, headers: &[(&str, &str)]) -> Result<Response, String> {
    CLIENT.put(url, body, content_type, headers)
}

pub fn delete(url: &str, headers: &[(&str, &str)]) -> Result<Response, String> {
    CLIENT.delete(url, headers)
}

pub fn delete_with_body(url: &str, body: &[u8], content_type: &str, headers: &[(&str, &str)]) -> Result<Response, String> {
    CLIENT.delete_with_body(url, body, content_type, headers)
}

pub fn patch(url: &str, body: &[u8], content_type: &str, headers: &[(&str, &str)]) -> Result<Response, String> {
    CLIENT.patch(url, body, content_type, headers)
}
