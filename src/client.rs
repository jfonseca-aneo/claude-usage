use anyhow::{bail, Context, Result};
use reqwest::blocking::Client;
use reqwest::Url;

use crate::models::{CostBucket, CostReport, OrgInfo, UsageBucket, UsageReport};

const BASE_URL: &str = "https://api.anthropic.com";
const ANTHROPIC_VERSION: &str = "2023-06-01";

pub struct AdminClient {
    http: Client,
    api_key: String,
}

impl AdminClient {
    pub fn new(api_key: String) -> Result<Self> {
        let http = Client::builder()
            .user_agent(concat!("claude-usage/", env!("CARGO_PKG_VERSION")))
            .build()
            .context("failed to build HTTP client")?;
        Ok(Self { http, api_key })
    }

    fn get(&self, url: Url) -> Result<reqwest::blocking::Response> {
        let resp = self
            .http
            .get(url.clone())
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .send()
            .with_context(|| format!("request to {url} failed"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().unwrap_or_default();
            bail!(
                "Anthropic API returned {status} for {url}\n{body}\n\n\
                 Hint: this endpoint requires an Admin API key (starts with \
                 sk-ant-admin...), not a regular API key. See \
                 https://platform.claude.com/docs/en/manage-claude/admin-api-keys"
            );
        }
        Ok(resp)
    }

    pub fn org_info(&self) -> Result<OrgInfo> {
        let url = Url::parse(BASE_URL)?.join("/v1/organizations/me")?;
        let info = self.get(url)?.json::<OrgInfo>()?;
        Ok(info)
    }

    /// Fetch every page of the messages usage report for the given window.
    pub fn usage_report(
        &self,
        starting_at: &str,
        ending_at: &str,
        bucket_width: &str,
        group_by: &[&str],
    ) -> Result<Vec<UsageBucket>> {
        let mut all = Vec::new();
        let mut page: Option<String> = None;
        loop {
            let mut url = Url::parse(BASE_URL)?.join("/v1/organizations/usage_report/messages")?;
            {
                let mut qp = url.query_pairs_mut();
                qp.append_pair("starting_at", starting_at);
                qp.append_pair("ending_at", ending_at);
                qp.append_pair("bucket_width", bucket_width);
                qp.append_pair("limit", max_limit_for_bucket(bucket_width));
                for g in group_by {
                    qp.append_pair("group_by[]", g);
                }
                if let Some(p) = &page {
                    qp.append_pair("page", p);
                }
            }
            let report = self.get(url)?.json::<UsageReport>()?;
            let has_more = report.has_more;
            let next_page = report.next_page;
            all.extend(report.data);
            if !has_more {
                break;
            }
            page = next_page;
            if page.is_none() {
                break;
            }
        }
        Ok(all)
    }

    /// Fetch every page of the cost report for the given window.
    pub fn cost_report(
        &self,
        starting_at: &str,
        ending_at: &str,
        group_by: &[&str],
    ) -> Result<Vec<CostBucket>> {
        let mut all = Vec::new();
        let mut page: Option<String> = None;
        loop {
            let mut url = Url::parse(BASE_URL)?.join("/v1/organizations/cost_report")?;
            {
                let mut qp = url.query_pairs_mut();
                qp.append_pair("starting_at", starting_at);
                qp.append_pair("ending_at", ending_at);
                qp.append_pair("limit", "31");
                for g in group_by {
                    qp.append_pair("group_by[]", g);
                }
                if let Some(p) = &page {
                    qp.append_pair("page", p);
                }
            }
            let report = self.get(url)?.json::<CostReport>()?;
            let has_more = report.has_more;
            let next_page = report.next_page;
            all.extend(report.data);
            if !has_more {
                break;
            }
            page = next_page;
            if page.is_none() {
                break;
            }
        }
        Ok(all)
    }
}

fn max_limit_for_bucket(bucket_width: &str) -> &'static str {
    match bucket_width {
        "1h" => "168",
        "1m" => "1440",
        _ => "31",
    }
}
