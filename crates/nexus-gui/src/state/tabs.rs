//! Open request tabs.

use super::response::Resp;
use super::ui_state::SavedTab;
use nexus_core::types::{create_default_request, NexusRequest};
use serde::{Deserialize, Serialize};

/// Where a tab's request is stored in a collection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Origin {
    pub dir_name: String,
    pub request_id: String,
}

pub struct RequestTab {
    /// Opaque tab id (initially the request id; it stays when the request is saved under a new id).
    pub id: String,
    pub request: NexusRequest,
    pub origin: Option<Origin>,
    pub dirty: bool,
    pub response: Option<Resp>,
    pub loading: bool,
    /// Aborts the in-flight send.
    pub abort: Option<tokio::task::AbortHandle>,
}

impl RequestTab {
    pub fn new(mut request: NexusRequest, origin: Option<Origin>) -> Self {
        // Requests saved with the query only in `params` get it in the URL too (two-way sync).
        let url = crate::url_sync::effective_url(&request.url, &request.params);
        if url != request.url {
            request.url = url;
            request.params = crate::url_sync::params_from_url(&request.url, &request.params);
        }
        Self {
            id: request.id.clone(),
            request,
            origin,
            dirty: false,
            response: None,
            loading: false,
            abort: None,
        }
    }

    pub fn to_saved(&self) -> SavedTab {
        SavedTab {
            request: self.request.clone(),
            origin: self.origin.clone(),
            dirty: self.dirty,
        }
    }

    /// Tab title: the name, or the URL for an unnamed request.
    pub fn label(&self) -> String {
        if self.request.name != "New Request" || self.request.url.is_empty() {
            self.request.name.clone()
        } else {
            self.request.url.clone()
        }
    }
}

#[derive(Default)]
pub struct Tabs {
    pub tabs: Vec<RequestTab>,
    active: Option<String>,
}

impl Tabs {
    pub fn from_saved(saved: &[SavedTab], active: Option<&str>) -> Self {
        let tabs: Vec<RequestTab> = saved
            .iter()
            .map(|s| {
                let mut t = RequestTab::new(s.request.clone(), s.origin.clone());
                t.dirty = s.dirty;
                t
            })
            .collect();
        let active = active
            .filter(|a| tabs.iter().any(|t| t.id == *a))
            .map(str::to_string)
            .or_else(|| tabs.last().map(|t| t.id.clone()));
        Self { tabs, active }
    }

    pub fn active_id(&self) -> Option<&str> {
        self.active.as_deref()
    }

    pub fn active_index(&self) -> Option<usize> {
        let a = self.active.as_deref()?;
        self.tabs.iter().position(|t| t.id == a)
    }

    pub fn active(&self) -> Option<&RequestTab> {
        self.active_index().map(|i| &self.tabs[i])
    }

    pub fn active_mut(&mut self) -> Option<&mut RequestTab> {
        self.active_index().map(|i| &mut self.tabs[i])
    }

    pub fn by_id_mut(&mut self, id: &str) -> Option<&mut RequestTab> {
        self.tabs.iter_mut().find(|t| t.id == id)
    }

    pub fn set_active(&mut self, id: &str) {
        if self.tabs.iter().any(|t| t.id == id) {
            self.active = Some(id.to_string());
        }
    }

    /// Opens `request` in a new tab. A request that is stored in a collection and already open
    /// just gets focused. Returns the tab id.
    pub fn open(&mut self, request: NexusRequest, origin: Option<Origin>) -> String {
        if let Some(o) = &origin {
            if let Some(t) = self.tabs.iter().find(|t| t.origin.as_ref() == Some(o)) {
                let id = t.id.clone();
                self.active = Some(id.clone());
                return id;
            }
        }
        let tab = RequestTab::new(request, origin);
        let id = tab.id.clone();
        self.tabs.push(tab);
        self.active = Some(id.clone());
        id
    }

    pub fn open_blank(&mut self) -> String {
        self.open(create_default_request(), None)
    }

    /// Closes a tab; the last remaining tab becomes active if the closed one was.
    pub fn close(&mut self, id: &str) {
        if let Some(a) = self
            .tabs
            .iter()
            .find(|t| t.id == id)
            .and_then(|t| t.abort.as_ref())
        {
            a.abort();
        }
        self.tabs.retain(|t| t.id != id);
        if self.active.as_deref() == Some(id) {
            self.active = self.tabs.last().map(|t| t.id.clone());
        }
    }

    /// Drops tabs whose collection request no longer exists (after delete/rename of a collection).
    pub fn close_origin(&mut self, dir_name: &str, request_id: Option<&str>) {
        let ids: Vec<String> = self
            .tabs
            .iter()
            .filter(|t| {
                t.origin.as_ref().is_some_and(|o| {
                    o.dir_name == dir_name && request_id.is_none_or(|r| r == o.request_id)
                })
            })
            .map(|t| t.id.clone())
            .collect();
        for id in ids {
            self.close(&id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_focuses_existing_origin_and_close_picks_last() {
        let mut t = Tabs::default();
        let r = create_default_request();
        let o = Origin {
            dir_name: "d".into(),
            request_id: r.id.clone(),
        };
        let a = t.open(r.clone(), Some(o.clone()));
        let b = t.open_blank();
        assert_eq!(t.active_id(), Some(b.as_str()));
        assert_eq!(t.open(r, Some(o)), a);
        assert_eq!(t.tabs.len(), 2);
        assert_eq!(t.active_id(), Some(a.as_str()));
        t.close(&a);
        assert_eq!(t.active_id(), Some(b.as_str()));
        t.close(&b);
        assert!(t.active().is_none());
    }

    #[test]
    fn unnamed_tab_is_labelled_by_url() {
        let mut r = create_default_request();
        r.url = "https://x.test".into();
        assert_eq!(RequestTab::new(r, None).label(), "https://x.test");
    }
}
