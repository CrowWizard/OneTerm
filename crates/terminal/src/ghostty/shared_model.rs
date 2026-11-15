//! Native shared model staging. Published frames are prepared on the dispatcher;
//! model reads never enter native code or invent legacy history/mode values.

use std::sync::Arc;

use crate::ghostty::{anchors, owner, search, state};
use crate::ghostty_bridge::{Client, Command, Error, Reply, Shared, State};
use crate::ghostty_projection::ProjectedFrame;

#[derive(Clone)]
pub struct SharedModel {
    client: Client,
}

impl SharedModel {
    /// Read one prepared publication without a damage watermark or native call.
    pub fn query(&self) -> crate::ghostty_content::Query {
        let publication = self.client.shared.read();
        crate::ghostty_content::Query {
            viewport: publication
                .frame
                .clone()
                .map(crate::ghostty_content::Viewport),
            state: publication.state.clone(),
            grid_revision: publication.grid_revision.clone(),
        }
    }

    pub(crate) fn new(client: Client) -> Self {
        Self { client }
    }

    pub fn reader(&self) -> ModelReader {
        ModelReader {
            shared: self.client.shared.clone(),
            previous: None,
        }
    }

    /// Low-level mutation only: does not publish or request repaint. Session UI
    /// must use its backend's queued scroll operation, not bypass pump delivery.
    pub async fn scroll(&self, scroll: state::Scroll) -> Result<(), Error> {
        self.done(Command::Scroll(scroll)).await
    }

    pub async fn create_anchor(&self, point: anchors::Point) -> Result<anchors::AnchorId, Error> {
        match self
            .client
            .request(Command::CreateAnchor(point))?
            .wait()
            .await?
        {
            Reply::Anchor(id) => Ok(id),
            _ => unreachable!("create-anchor reply matches its private dispatch"),
        }
    }

    pub async fn resolve_anchor(
        &self,
        id: &anchors::AnchorId,
        space: anchors::Space,
    ) -> Result<Option<anchors::Position>, Error> {
        match self
            .client
            .request(Command::ResolveAnchor(id.clone(), space))?
            .wait()
            .await?
        {
            Reply::Position(position) => Ok(position),
            _ => unreachable!("resolve-anchor reply matches its private dispatch"),
        }
    }

    pub async fn release_anchor(&self, id: &anchors::AnchorId) -> Result<(), Error> {
        self.done(Command::ReleaseAnchor(id.clone())).await
    }

    /// Low-level mutation without publication. Session consumers should use
    /// their queued selection update to publish selected-cell frames safely.
    pub async fn set_selection(
        &self,
        start: &anchors::AnchorId,
        end: &anchors::AnchorId,
        rectangle: bool,
    ) -> Result<(), Error> {
        self.done(Command::SetSelection(start.clone(), end.clone(), rectangle))
            .await
    }

    /// Low-level mutation only; does not capture or request repaint.
    pub async fn clear_selection(&self) -> Result<(), Error> {
        self.done(Command::ClearSelection).await
    }

    pub async fn selection_text(&self) -> Result<Option<String>, Error> {
        match self.client.request(Command::SelectionText)?.wait().await? {
            Reply::Text(text) => Ok(text),
            _ => unreachable!("selection-text reply matches its private dispatch"),
        }
    }

    pub async fn search(
        &self,
        needle: String,
        options: search::Options,
    ) -> Result<search::Snapshot, Error> {
        self.search_request(Command::Search(needle, options)).await
    }

    pub async fn search_step(&self) -> Result<search::Snapshot, Error> {
        self.search_request(Command::SearchStep).await
    }

    async fn search_request(&self, command: Command) -> Result<search::Snapshot, Error> {
        match self.client.request(command)?.wait().await? {
            Reply::Search(snapshot) => Ok(snapshot),
            _ => unreachable!("search reply matches its private dispatch"),
        }
    }

    async fn done(&self, command: Command) -> Result<(), Error> {
        match self.client.request(command)?.wait().await? {
            Reply::Done => Ok(()),
            _ => unreachable!("mutation reply matches its private dispatch"),
        }
    }
}

pub struct ModelReader {
    shared: Arc<Shared>,
    previous: Option<Arc<ProjectedFrame>>,
}

pub struct Update {
    pub frame: Option<Arc<ProjectedFrame>>,
    pub damage: owner::Damage,
    pub state: State,
}

impl ModelReader {
    pub(crate) fn is_source(&self, model: &SharedModel) -> bool {
        Arc::ptr_eq(&self.shared, &model.client.shared)
    }

    /// Only an Arc clone under a pointer lock, then consumer-owned comparisons.
    /// No native call or projection runs here. Row damage may allocate, as in the
    /// existing staged FrameReader; this is not a zero-allocation render claim.
    pub fn poll(&mut self) -> Result<Update, Error> {
        let (frame, state) = {
            let publication = self.shared.read();
            (publication.frame.clone(), publication.state.clone())
        };
        let damage = match (&self.previous, &frame) {
            (_, None) => owner::Damage::None,
            (None, Some(_)) => owner::Damage::Full,
            (Some(before), Some(after)) if Arc::ptr_eq(before, after) => owner::Damage::None,
            (Some(before), Some(after)) => {
                owner::frame_damage(before.source(), after.source()).map_err(Error::Projection)?
            }
        };
        self.previous = frame.clone();
        Ok(Update {
            frame,
            damage,
            state,
        })
    }
}
