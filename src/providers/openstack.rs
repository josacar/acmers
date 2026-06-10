use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Openstack {
    auth_url: String,
    auth_type: String,
    username: Option<String>,
    password: Option<String>,
    project_name: Option<String>,
    project_id: Option<String>,
    user_domain_name: Option<String>,
    user_domain_id: Option<String>,
    project_domain_name: Option<String>,
    project_domain_id: Option<String>,
    app_cred_id: Option<String>,
    app_cred_secret: Option<String>,
}

struct AuthResult {
    token: String,
    designate_url: String,
}

impl DnsProvider for Openstack {
    fn slug() -> &'static str {
        "openstack"
    }

    fn env_vars() -> &'static [&'static str] {
        &[
            "OS_AUTH_URL", "OS_USERNAME", "OS_PASSWORD", "OS_PROJECT_NAME",
            "OS_PROJECT_DOMAIN_NAME", "OS_USER_DOMAIN_NAME", "OS_AUTH_TYPE",
            "OS_APPLICATION_CREDENTIAL_ID", "OS_APPLICATION_CREDENTIAL_SECRET",
            "OS_PROJECT_ID", "OS_USER_DOMAIN_ID", "OS_PROJECT_DOMAIN_ID",
        ]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let auth_url = env.get("OS_AUTH_URL")
            .ok_or_else(|| Error::Config("OS_AUTH_URL required".into()))?
            .clone();
        let auth_type = env.get("OS_AUTH_TYPE").cloned().unwrap_or_default();

        if auth_type == "v3applicationcredential" {
            let app_cred_id = env.get("OS_APPLICATION_CREDENTIAL_ID")
                .ok_or_else(|| Error::Config("OS_APPLICATION_CREDENTIAL_ID required for application credential auth".into()))?
                .clone();
            let app_cred_secret = env.get("OS_APPLICATION_CREDENTIAL_SECRET")
                .ok_or_else(|| Error::Config("OS_APPLICATION_CREDENTIAL_SECRET required for application credential auth".into()))?
                .clone();
            Ok(Box::new(Openstack {
                auth_url: auth_url.trim_end_matches('/').to_string(),
                auth_type,
                username: None, password: None,
                project_name: None, project_id: None,
                user_domain_name: None, user_domain_id: None,
                project_domain_name: None, project_domain_id: None,
                app_cred_id: Some(app_cred_id),
                app_cred_secret: Some(app_cred_secret),
            }))
        } else {
            let username = env.get("OS_USERNAME")
                .ok_or_else(|| Error::Config("OS_USERNAME required".into()))?
                .clone();
            let password = env.get("OS_PASSWORD")
                .ok_or_else(|| Error::Config("OS_PASSWORD required".into()))?
                .clone();
            let project_name = env.get("OS_PROJECT_NAME").cloned();
            let project_id = env.get("OS_PROJECT_ID").cloned();
            if project_name.is_none() && project_id.is_none() {
                return Err(Error::Config("OS_PROJECT_NAME or OS_PROJECT_ID required".into()));
            }
            Ok(Box::new(Openstack {
                auth_url: auth_url.trim_end_matches('/').to_string(),
                auth_type,
                username: Some(username),
                password: Some(password),
                project_name, project_id,
                user_domain_name: env.get("OS_USER_DOMAIN_NAME").cloned(),
                user_domain_id: env.get("OS_USER_DOMAIN_ID").cloned(),
                project_domain_name: env.get("OS_PROJECT_DOMAIN_NAME").cloned(),
                project_domain_id: env.get("OS_PROJECT_DOMAIN_ID").cloned(),
                app_cred_id: None, app_cred_secret: None,
            }))
        }
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let auth = self.authenticate()?;
        let fulldomain = format!("{}.", name);
        let zone_id = self.find_zone(&fulldomain, &auth)?;
        let recordset_id = self.find_recordset(&zone_id, &fulldomain, &auth)?;

        let recordset_id = if let Some(ref rs_id) = recordset_id {
            let existing = self.get_records(&zone_id, rs_id, &auth)?;
            let mut records = existing;
            let quoted = format!("\"{}\"", value);
            if records.contains(&quoted) {
                return Ok(());
            }
            records.push(quoted);
            self.update_recordset(&zone_id, rs_id, &fulldomain, &records, &auth)?
        } else {
            let quoted = format!("\"{}\"", value);
            self.create_recordset(&zone_id, &fulldomain, &[quoted], &auth)?
        };

        self.wait_active(&zone_id, &recordset_id, &auth)?;
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let auth = match self.authenticate() {
            Ok(a) => a,
            Err(_) => return Ok(()),
        };
        let fulldomain = format!("{}.", name);
        let zone_id = match self.find_zone(&fulldomain, &auth) {
            Ok(z) => z,
            Err(_) => return Ok(()),
        };
        let recordset_id = match self.find_recordset(&zone_id, &fulldomain, &auth) {
            Ok(Some(id)) => id,
            Ok(None) => return Ok(()),
            Err(_) => return Ok(()),
        };
        let records = match self.get_records(&zone_id, &recordset_id, &auth) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let quoted = format!("\"{}\"", value);
        if records.len() == 1 && records[0] == quoted {
            let _ = self.delete_recordset(&zone_id, &recordset_id, &auth);
        } else {
            let remaining: Vec<String> = records.into_iter()
                .filter(|r| r != &quoted)
                .collect();
            let _ = self.update_recordset(&zone_id, &recordset_id, &fulldomain, &remaining, &auth);
        }
        Ok(())
    }
}

impl Openstack {
    fn authenticate(&self) -> Result<AuthResult, Error> {
        let url = format!("{}/v3/auth/tokens", self.auth_url);
        let body = if self.auth_type == "v3applicationcredential" {
            serde_json::json!({
                "auth": {
                    "identity": {
                        "methods": ["application_credential"],
                        "application_credential": {
                            "id": self.app_cred_id.as_ref().unwrap(),
                            "secret": self.app_cred_secret.as_ref().unwrap(),
                        }
                    }
                }
            })
        } else {
            let mut user = serde_json::json!({
                "name": self.username.as_ref().unwrap(),
                "password": self.password.as_ref().unwrap(),
            });
            if let Some(ref domain_name) = self.user_domain_name {
                user["domain"] = serde_json::json!({"name": domain_name});
            } else if let Some(ref domain_id) = self.user_domain_id {
                user["domain"] = serde_json::json!({"id": domain_id});
            }

            let mut project = serde_json::Map::new();
            if let Some(ref name) = self.project_name {
                project.insert("name".into(), serde_json::json!(name));
            } else if let Some(ref id) = self.project_id {
                project.insert("id".into(), serde_json::json!(id));
            }
            if let Some(ref domain_name) = self.project_domain_name {
                project.insert("domain".into(), serde_json::json!({"name": domain_name}));
            } else if let Some(ref domain_id) = self.project_domain_id {
                project.insert("domain".into(), serde_json::json!({"id": domain_id}));
            }

            serde_json::json!({
                "auth": {
                    "identity": {
                        "methods": ["password"],
                        "password": {
                            "user": user
                        }
                    },
                    "scope": {
                        "project": project
                    }
                }
            })
        };

        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json", &[])
            .map_err(|e| Error::Provider(format!("openstack keystone auth: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("openstack keystone auth: HTTP {} {}", resp.status, resp.body)));
        }

        let token = resp.headers.get("x-subject-token")
            .cloned()
            .ok_or_else(|| Error::Provider("openstack keystone auth: missing x-subject-token header".into()))?;

        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("openstack keystone auth parse: {e}")))?;

        let designate_url = find_service_endpoint(&v, "dns")
            .ok_or_else(|| Error::Provider("openstack: Designate (dns) endpoint not found in service catalog".into()))?;

        Ok(AuthResult { token, designate_url: designate_url.trim_end_matches('/').to_string() })
    }

    fn headers<'a>(&self, auth: &'a AuthResult) -> Vec<(&'a str, &'a str)> {
        vec![("X-Auth-Token", auth.token.as_str())]
    }

    fn find_zone(&self, fulldomain: &str, auth: &AuthResult) -> Result<String, Error> {
        let url = format!("{}/v2/zones?limit=2000", auth.designate_url);
        let headers = self.headers(auth);
        let resp = http::get(&url, &headers)
            .map_err(|e| Error::Provider(format!("openstack list zones: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("openstack list zones: HTTP {} {}", resp.status, resp.body)));
        }
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("openstack list zones: {e}")))?;
        let zones = v.get("zones").and_then(|z| z.as_array())
            .ok_or_else(|| Error::Provider("openstack: unexpected zone list response".into()))?;

        let parts: Vec<&str> = fulldomain.split('.').collect();
        for i in 1..parts.len() {
            let candidate = format!("{}.", parts[i..].join("."));
            for z in zones {
                if z.get("name").and_then(|n| n.as_str()) == Some(&candidate) {
                    return z.get("id").and_then(|id| id.as_str()).map(|s| s.to_string())
                        .ok_or_else(|| Error::Provider("openstack: zone id not found".into()));
                }
            }
        }
        Err(Error::Provider(format!("openstack: no matching zone found for {fulldomain}")))
    }

    fn find_recordset(&self, zone_id: &str, fulldomain: &str, auth: &AuthResult) -> Result<Option<String>, Error> {
        let url = format!("{}/v2/zones/{}/recordsets?name={}", auth.designate_url, zone_id, fulldomain);
        let headers = self.headers(auth);
        let resp = http::get(&url, &headers)
            .map_err(|e| Error::Provider(format!("openstack list recordsets: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("openstack list recordsets: HTTP {} {}", resp.status, resp.body)));
        }
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("openstack list recordsets: {e}")))?;
        if let Some(recordsets) = v.get("recordsets").and_then(|r| r.as_array()) {
            for rs in recordsets {
                if rs.get("name").and_then(|n| n.as_str()) == Some(fulldomain)
                    && rs.get("type").and_then(|t| t.as_str()) == Some("TXT")
                {
                    return Ok(rs.get("id").and_then(|id| id.as_str()).map(|s| s.to_string()));
                }
            }
        }
        Ok(None)
    }

    fn get_records(&self, zone_id: &str, recordset_id: &str, auth: &AuthResult) -> Result<Vec<String>, Error> {
        let url = format!("{}/v2/zones/{}/recordsets/{}", auth.designate_url, zone_id, recordset_id);
        let headers = self.headers(auth);
        let resp = http::get(&url, &headers)
            .map_err(|e| Error::Provider(format!("openstack get recordset: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("openstack get recordset: HTTP {} {}", resp.status, resp.body)));
        }
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("openstack get recordset: {e}")))?;
        let records = v.get("records").and_then(|r| r.as_array())
            .ok_or_else(|| Error::Provider("openstack: no records field in recordset".into()))?;
        Ok(records.iter().filter_map(|r| r.as_str().map(|s| s.to_string())).collect())
    }

    fn create_recordset(&self, zone_id: &str, fulldomain: &str, records: &[String], auth: &AuthResult) -> Result<String, Error> {
        let url = format!("{}/v2/zones/{}/recordsets", auth.designate_url, zone_id);
        let headers = self.headers(auth);
        let body = serde_json::json!({
            "name": fulldomain,
            "type": "TXT",
            "records": records,
        });
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json", &headers)
            .map_err(|e| Error::Provider(format!("openstack create recordset: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("openstack create recordset: HTTP {} {}", resp.status, resp.body)));
        }
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("openstack create recordset: {e}")))?;
        v.get("id").and_then(|id| id.as_str()).map(|s| s.to_string())
            .ok_or_else(|| Error::Provider("openstack: no id in create recordset response".into()))
    }

    fn update_recordset(&self, zone_id: &str, recordset_id: &str, fulldomain: &str, records: &[String], auth: &AuthResult) -> Result<String, Error> {
        let url = format!("{}/v2/zones/{}/recordsets/{}", auth.designate_url, zone_id, recordset_id);
        let headers = self.headers(auth);
        let body = serde_json::json!({
            "name": fulldomain,
            "type": "TXT",
            "records": records,
        });
        let resp = http::put(&url, &serde_json::to_vec(&body).unwrap(), "application/json", &headers)
            .map_err(|e| Error::Provider(format!("openstack update recordset: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("openstack update recordset: HTTP {} {}", resp.status, resp.body)));
        }
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("openstack update recordset: {e}")))?;
        v.get("id").and_then(|id| id.as_str()).map(|s| s.to_string())
            .ok_or_else(|| Error::Provider("openstack: no id in update recordset response".into()))
    }

    fn delete_recordset(&self, zone_id: &str, recordset_id: &str, auth: &AuthResult) -> Result<(), Error> {
        let url = format!("{}/v2/zones/{}/recordsets/{}", auth.designate_url, zone_id, recordset_id);
        let headers = self.headers(auth);
        let resp = http::delete(&url, &headers)
            .map_err(|e| Error::Provider(format!("openstack delete recordset: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("openstack delete recordset: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn wait_active(&self, zone_id: &str, recordset_id: &str, auth: &AuthResult) -> Result<(), Error> {
        let url = format!("{}/v2/zones/{}/recordsets/{}", auth.designate_url, zone_id, recordset_id);
        let headers = self.headers(auth);
        for _ in 0..60 {
            let resp = http::get(&url, &headers)
                .map_err(|e| Error::Provider(format!("openstack check status: {e}")))?;
            if resp.status < 400 {
                if let Ok(v) = serde_json::from_str::<Value>(&resp.body) {
                    let status = v.get("status").and_then(|s| s.as_str()).unwrap_or("");
                    if status == "ACTIVE" {
                        return Ok(());
                    }
                    if status == "ERROR" {
                        return Err(Error::Provider("openstack: recordset entered ERROR state".into()));
                    }
                }
            }
            std::thread::sleep(std::time::Duration::from_secs(5));
        }
        Err(Error::Provider("openstack: recordset did not become ACTIVE within timeout".into()))
    }
}

fn find_service_endpoint(catalog: &Value, service_type: &str) -> Option<String> {
    let token_data = catalog.get("token")?;
    let catalog = token_data.get("catalog")?.as_array()?;
    for service in catalog {
        if service.get("type").and_then(|t| t.as_str()) == Some(service_type) {
            let endpoints = service.get("endpoints")?.as_array()?;
            for ep in endpoints {
                let iface = ep.get("interface").and_then(|i| i.as_str()).unwrap_or("");
                if iface == "public" {
                    return ep.get("url").and_then(|u| u.as_str()).map(|s| s.to_string());
                }
            }
            if let Some(first) = endpoints.first() {
                return first.get("url").and_then(|u| u.as_str()).map(|s| s.to_string());
            }
        }
    }
    None
}
